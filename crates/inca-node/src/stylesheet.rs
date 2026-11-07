// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The parse of the text of a `<style>` element into rules, and the warnings for what it drops.

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, ParseErrorKind, Parser, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
};

use crate::selector::Selectors;
use crate::spec::{style_key, style_warning};
use crate::tree::AttributeValue;

/// One qualified rule: its selectors and its declarations in source order.
#[derive(Debug)]
pub(crate) struct Rule {
    pub(crate) selectors: Selectors,
    pub(crate) declarations: Vec<(String, AttributeValue)>,
}

/// The rules of one `<style>` element.
#[derive(Debug, Default)]
pub(crate) struct StyleSheet {
    pub(crate) rules: Vec<Rule>,
}

/// The declarations of one rule block and the warnings its invalid declarations raise.
#[derive(Default)]
struct Body {
    declarations: Vec<(String, AttributeValue)>,
    warnings: Vec<String>,
}

/// `text` without its `/* */` comments.
fn without_comments(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        rest = after.find("*/").map_or("", |close| &after[close + 2..]);
    }
    out.push_str(rest);
    out
}

/// The value without a trailing `!important`.
fn without_important(value: &str) -> &str {
    match value.rsplit_once('!') {
        Some((head, flag)) if flag.trim_ascii().eq_ignore_ascii_case("important") => {
            head.trim_ascii_end()
        }
        _ => value,
    }
}

impl<'i> DeclarationParser<'i> for Body {
    type Declaration = ();
    type Error = &'static str;

    fn parse_value(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
        _start: &ParserState,
    ) -> Result<(), ParseError<&'static str>> {
        let start = input.position();
        while input.next_including_whitespace_and_comments().is_ok() {}
        let source = without_comments(input.slice_from(start));
        let text = without_important(source.trim_ascii());
        if text.is_empty() {
            return Ok(());
        }
        let name = name.to_ascii_lowercase();
        let value = AttributeValue::String(text.to_owned());
        match style_warning(&name, &value) {
            Some(warning) => self.warnings.push(warning),
            None => self
                .declarations
                .push((style_key(&name).into_owned(), value)),
        }
        Ok(())
    }
}

impl AtRuleParser<'_> for Body {
    type Prelude = ();
    type AtRule = ();
    type Error = &'static str;
}

impl QualifiedRuleParser<'_> for Body {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = &'static str;
}

impl RuleBodyItemParser<'_, (), &'static str> for Body {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

/// Reads the qualified rules of a sheet. An at-rule keeps the trait's default and is rejected.
struct Rules;

impl<'i> QualifiedRuleParser<'i> for Rules {
    type Prelude = Selectors;
    type QualifiedRule = (Rule, Vec<String>);
    type Error = &'static str;

    fn parse_prelude(
        &mut self,
        input: &mut Parser<'i>,
    ) -> Result<Selectors, ParseError<&'static str>> {
        let start = input.position();
        while input.next_including_whitespace_and_comments().is_ok() {}
        let text = input.slice_from(start);
        Selectors::parse(text).map_err(|error| ParseError::custom(error.reason()))
    }

    fn parse_block(
        &mut self,
        selectors: Selectors,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<(Rule, Vec<String>), ParseError<&'static str>> {
        let mut body = Body::default();
        let mut invalid = Vec::new();
        for item in RuleBodyParser::new(input, &mut body) {
            if let Err((_, source, _)) = item {
                invalid.push(format!(
                    "ignoring the declaration `{}`",
                    source.trim_end_matches(|c: char| c == ';' || c.is_ascii_whitespace())
                ));
            }
        }
        let Body {
            declarations,
            mut warnings,
        } = body;
        warnings.extend(invalid);
        Ok((
            Rule {
                selectors,
                declarations,
            },
            warnings,
        ))
    }
}

impl AtRuleParser<'_> for Rules {
    type Prelude = ();
    type AtRule = (Rule, Vec<String>);
    type Error = &'static str;
}

/// The name of the at-rule that `source` starts with, `@` included.
fn at_rule_name(source: &str) -> &str {
    let end = source[1..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .map_or(source.len(), |end| end + 1);
    &source[..end]
}

impl StyleSheet {
    /// The rules of `text` and one line for each rule, at-rule or declaration it drops.
    pub(crate) fn parse(text: &str) -> (Self, Vec<String>) {
        let mut input = Parser::new(text);
        let mut rules = Vec::new();
        let mut warnings = Vec::new();
        for item in StyleSheetParser::new(&mut input, &mut Rules) {
            match item {
                Ok((rule, dropped)) => {
                    rules.push(rule);
                    warnings.extend(dropped);
                }
                Err((_, source, _)) if source.starts_with('@') => {
                    warnings.push(format!("ignoring the at-rule `{}`", at_rule_name(source)));
                }
                Err((error, source, _)) => {
                    let prelude = source.split('{').next().unwrap_or(source).trim_ascii();
                    let reason = match error.kind {
                        ParseErrorKind::Custom(reason) => reason,
                        ParseErrorKind::Basic(_) => "it has no block",
                    };
                    warnings.push(format!("ignoring the rule `{prelude}`: {reason}"));
                }
            }
        }
        (Self { rules }, warnings)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::tests::{Fixture, text};

    #[test]
    fn an_invalid_declaration_keeps_the_earlier_value_of_its_key() {
        let mut f = Fixture::new();
        f.sheet(".a { width: 10px; width: bogus }");
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), Some(text("10px")));
    }

    #[test]
    fn an_invalid_declaration_warns() {
        let mut f = Fixture::new();
        f.sheet(".a { width: bogus }");

        assert_eq!(
            f.tree.take_style_warnings(),
            ["ignoring unsupported value for style key `width`"]
        );
    }

    #[test]
    fn a_declaration_without_a_colon_warns() {
        let mut f = Fixture::new();
        f.sheet(".a { width 10px; height: 2px }");

        assert_eq!(
            f.tree.take_style_warnings(),
            ["ignoring the declaration `width 10px`"]
        );
    }

    #[test]
    fn a_declaration_after_one_without_a_colon_still_applies() {
        let mut f = Fixture::new();
        f.sheet(".a { width 10px; height: 2px }");
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "height"), Some(text("2px")));
    }

    #[test]
    fn a_declaration_name_ignores_case() {
        let mut f = Fixture::new();
        f.sheet(".a { WIDTH: 1px }");
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), Some(text("1px")));
    }

    #[test]
    fn an_empty_value_is_dropped_silently() {
        let mut f = Fixture::new();
        f.sheet(".a { width: ; height: 2px }");
        let a = f.boxed("a");

        assert!(f.tree.take_style_warnings().is_empty());
        assert_eq!(f.computed(a, "width"), None);
    }

    #[rstest]
    #[case::around_the_value(".a { width: /* c */ 10px /* d */ }")]
    #[case::unterminated(".a { width: 10px /* c }")]
    #[case::before_important(".a { width: 10px /* c */ !important }")]
    fn a_comment_inside_a_value_is_skipped(#[case] css: &str) {
        let mut f = Fixture::new();
        f.sheet(css);
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), Some(text("10px")));
    }

    #[test]
    fn a_comment_before_a_selector_is_skipped() {
        let mut f = Fixture::new();
        f.sheet("/* c */ .a { width: 1px }");
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), Some(text("1px")));
    }

    #[rstest]
    #[case::unknown_pseudo_class(
        "a:foo { width: 1px } .a { width: 2px }",
        "ignoring the rule `a:foo`: it names an unknown pseudo-class or pseudo-element"
    )]
    #[case::missing_block(".a { width: 2px } .c", "ignoring the rule `.c`: it has no block")]
    fn an_invalid_rule_warns(#[case] css: &str, #[case] warning: &str) {
        let mut f = Fixture::new();
        f.sheet(css);

        assert_eq!(f.tree.take_style_warnings(), [warning]);
    }

    #[test]
    fn a_rule_after_an_invalid_rule_still_applies() {
        let mut f = Fixture::new();
        f.sheet("a:foo { width: 1px } .a { width: 2px }");
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), Some(text("2px")));
    }

    #[test]
    fn an_at_rule_is_skipped_with_its_block() {
        let mut f = Fixture::new();
        f.sheet("@media (min-width: 1px) { .a { width: 1px } } .a { height: 2px }");
        let a = f.boxed("a");

        assert_eq!(
            f.tree.take_style_warnings(),
            ["ignoring the at-rule `@media`"]
        );
        assert_eq!(f.computed(a, "width"), None);
        assert_eq!(f.computed(a, "height"), Some(text("2px")));
    }

    #[test]
    fn important_applies_at_normal_importance() {
        let mut f = Fixture::new();
        f.sheet("#a { width: 5px !important } .a { width: 9px }");
        let a = f.boxed("a");

        assert_eq!(f.computed(a, "width"), Some(text("5px")));
    }
}
