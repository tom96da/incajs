// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The rules of the `<style>` elements and the cascade that merges them with a node's own style.

use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::rc::Rc;

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, ParseErrorKind, Parser, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
};

use super::{AttributeValue, NodeId, TEXT_TAG, VirtualNode, VirtualTree};
use crate::element::{style_key, style_warning};
use crate::selector::Selectors;

/// The longhands that a shorthand key overrides when it is applied after them.
const SHORTHANDS: &[(&str, &[&str])] = &[
    (
        "padding",
        &[
            "padding_x",
            "padding_y",
            "padding_top",
            "padding_right",
            "padding_bottom",
            "padding_left",
        ],
    ),
    ("padding_x", &["padding_left", "padding_right"]),
    ("padding_y", &["padding_top", "padding_bottom"]),
    (
        "margin",
        &[
            "margin_x",
            "margin_y",
            "margin_top",
            "margin_right",
            "margin_bottom",
            "margin_left",
        ],
    ),
    ("margin_x", &["margin_left", "margin_right"]),
    ("margin_y", &["margin_top", "margin_bottom"]),
    ("overflow", &["overflow_x", "overflow_y"]),
];

/// How many longhands `key` overrides: 0 for a key that is no shorthand.
fn covers(key: &str) -> usize {
    SHORTHANDS
        .iter()
        .find(|(shorthand, _)| *shorthand == key)
        .map_or(0, |(_, longhands)| longhands.len())
}

/// Sets `key`, after removing the keys it overrides.
fn apply(map: &mut HashMap<String, AttributeValue>, key: &str, value: &AttributeValue) {
    if let Some((_, longhands)) = SHORTHANDS.iter().find(|(shorthand, _)| *shorthand == key) {
        for longhand in *longhands {
            map.remove(*longhand);
        }
    }
    map.insert(key.to_owned(), value.clone());
}

/// The entries of `props`, shorthands before the longhands they override.
fn shorthands_first<'a>(
    props: impl IntoIterator<Item = (&'a String, &'a AttributeValue)>,
) -> Vec<(&'a String, &'a AttributeValue)> {
    let mut entries: Vec<_> = props.into_iter().collect();
    entries.sort_by_key(|(key, _)| (Reverse(covers(key)), key.as_str()));
    entries
}

/// One qualified rule: its selectors and its declarations in source order.
#[derive(Debug)]
struct Rule {
    selectors: Selectors,
    declarations: Vec<(String, AttributeValue)>,
}

/// The rules of one `<style>` element.
#[derive(Debug, Default)]
struct StyleSheet {
    rules: Vec<Rule>,
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
    fn parse(text: &str) -> (Self, Vec<String>) {
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

/// The sheets of the connected `<style>` elements, kept until the tree changes.
#[derive(Debug, Default)]
pub(super) struct SheetCache {
    /// The generation of the tree the cache was built for.
    generation: Option<u64>,
    /// The sheets in tree order.
    sheets: Vec<Rc<StyleSheet>>,
    /// The text each `<style>` element had when its sheet was parsed.
    parsed: HashMap<NodeId, (String, Rc<StyleSheet>)>,
    /// The warning lines of the parses nobody has taken yet.
    warnings: Vec<String>,
}

impl VirtualTree {
    /// The connected `<style>` elements in tree order. None while the tree has no root.
    fn style_elements(&self) -> Vec<NodeId> {
        let mut found = Vec::new();
        let mut pending: Vec<NodeId> = self.root.into_iter().collect();
        while let Some(id) = pending.pop() {
            let Some(node) = self.get(id) else { continue };
            if node.tag_name().eq_ignore_ascii_case("style") {
                found.push(id);
                // The children of a `<style>` element are its text.
                continue;
            }
            pending.extend(node.children().iter().rev());
        }
        found
    }

    /// The data of the text children of `id`, in order.
    fn child_text(&self, id: NodeId) -> String {
        self.get(id)
            .into_iter()
            .flat_map(VirtualNode::children)
            .filter_map(|&child| self.get(child))
            .filter(|child| child.tag_name() == TEXT_TAG)
            .map(VirtualNode::data)
            .collect()
    }

    /// Collects the sheets again when the tree changed. A sheet whose text is unchanged keeps its
    /// parse.
    fn refresh_sheets(&self) {
        let mut cache = self.style_cache.borrow_mut();
        if cache.generation == Some(self.generation) {
            return;
        }
        cache.generation = Some(self.generation);
        let mut sheets = Vec::new();
        let mut parsed = HashMap::new();
        for id in self.style_elements() {
            let text = self.child_text(id);
            let entry = match cache.parsed.remove(&id) {
                Some((old, sheet)) if old == text => (old, sheet),
                _ => {
                    let (sheet, warnings) = StyleSheet::parse(&text);
                    cache.warnings.extend(warnings);
                    (text, Rc::new(sheet))
                }
            };
            sheets.push(Rc::clone(&entry.1));
            parsed.insert(id, entry);
        }
        cache.sheets = sheets;
        cache.parsed = parsed;
    }

    /// The warning lines of the sheet parses since the last call. A sheet with unchanged text
    /// adds none.
    pub fn take_style_warnings(&self) -> Vec<String> {
        self.refresh_sheets();
        std::mem::take(&mut self.style_cache.borrow_mut().warnings)
    }

    /// The style of `id` after the rules of the connected `<style>` elements apply: the matching
    /// declarations by specificity, then sheet order, then rule order, and the node's own style
    /// last. A declaration of a shorthand replaces the longhands set before it. A node that no
    /// rule styles, an unknown id and a tree without a `<style>` element give the node's own map.
    #[must_use]
    pub(crate) fn computed_style(&self, id: NodeId) -> Cow<'_, HashMap<String, AttributeValue>> {
        let Some(node) = self.get(id) else {
            return Cow::Owned(HashMap::new());
        };
        self.refresh_sheets();
        let sheets = self.style_cache.borrow().sheets.clone();
        if sheets.is_empty() {
            return Cow::Borrowed(node.style_props());
        }
        let mut matched: Vec<(u32, usize, usize, &Rule)> = Vec::new();
        for (sheet_index, sheet) in sheets.iter().enumerate() {
            for (rule_index, rule) in sheet.rules.iter().enumerate() {
                if let Some(specificity) = rule.selectors.specificity_of_match(self, id) {
                    matched.push((specificity, sheet_index, rule_index, rule));
                }
            }
        }
        matched.sort_by_key(|&(specificity, sheet_index, rule_index, _)| {
            (specificity, sheet_index, rule_index)
        });
        if matched.is_empty() {
            return Cow::Borrowed(node.style_props());
        }
        let mut computed = HashMap::new();
        for (_, _, _, rule) in matched {
            for (key, value) in &rule.declarations {
                apply(&mut computed, key, value);
            }
        }
        for (key, value) in shorthands_first(node.style_props()) {
            apply(&mut computed, key, value);
        }
        Cow::Owned(computed)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::build_spec;
    use crate::tests::{Fixture, num, text};

    /// A `div` under the root with `name` as its id and its class.
    fn boxed(f: &mut Fixture, name: &str) -> NodeId {
        let node = f.add(f.root, "div", name);
        f.attr(node, "class", name);
        node
    }

    fn value(f: &Fixture, node: NodeId, key: &str) -> Option<AttributeValue> {
        f.tree.computed_style(node).get(key).cloned()
    }

    fn padding_top(f: &Fixture, node: NodeId) -> Option<f64> {
        build_spec(&f.tree, node).unwrap().style.padding.top
    }

    #[test]
    fn a_rule_applies_its_declarations_to_a_matching_node() {
        let mut f = Fixture::new();
        f.sheet(".a { width: 40px }");
        let a = boxed(&mut f, "a");
        let b = boxed(&mut f, "b");

        assert_eq!(value(&f, a, "width"), Some(text("40px")));
        assert_eq!(value(&f, b, "width"), None);
    }

    #[test]
    fn a_higher_specificity_beats_a_later_rule() {
        let mut f = Fixture::new();
        f.sheet("#a { width: 1px } .a { width: 2px }");
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), Some(text("1px")));
    }

    #[test]
    fn the_later_sheet_wins_at_equal_specificity() {
        let mut f = Fixture::new();
        f.sheet(".a { width: 1px }");
        f.sheet(".a { width: 2px }");
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), Some(text("2px")));
    }

    #[test]
    fn the_inline_style_beats_every_rule() {
        let mut f = Fixture::new();
        f.sheet("#a.a { width: 1px }");
        let a = boxed(&mut f, "a");
        f.tree.set_style(a, "width", 50.0).unwrap();

        assert_eq!(value(&f, a, "width"), Some(num(50.0)));
    }

    #[test]
    fn a_rule_shorthand_replaces_an_earlier_rule_longhand() {
        let mut f = Fixture::new();
        f.sheet(".a { padding-top: 8px } .a { padding: 4px }");
        let a = boxed(&mut f, "a");

        assert_eq!(padding_top(&f, a), Some(4.0));
    }

    #[test]
    fn an_inline_shorthand_replaces_a_rule_longhand() {
        let mut f = Fixture::new();
        f.sheet(".a { padding-top: 8px }");
        let a = boxed(&mut f, "a");
        f.tree.set_style(a, "padding", 4.0).unwrap();

        assert_eq!(padding_top(&f, a), Some(4.0));
    }

    #[test]
    fn an_inline_axis_replaces_a_rule_side() {
        let mut f = Fixture::new();
        f.sheet(".a { padding-left: 8px }");
        let a = boxed(&mut f, "a");
        f.tree.set_style(a, "padding_x", 4.0).unwrap();

        assert_eq!(
            build_spec(&f.tree, a).unwrap().style.padding.left,
            Some(4.0)
        );
    }

    #[rstest]
    #[case::margin("margin-top: 1px; margin: 2px", "margin_top")]
    #[case::margin_x("margin-left: 1px; margin-x: 2px", "margin_left")]
    #[case::margin_y("margin-bottom: 1px; margin-y: 2px", "margin_bottom")]
    #[case::padding_x("padding-left: 1px; padding-x: 2px", "padding_left")]
    #[case::padding_y("padding-top: 1px; padding-y: 2px", "padding_top")]
    #[case::overflow("overflow-x: hidden; overflow: scroll", "overflow_x")]
    fn a_later_shorthand_removes_its_longhand(#[case] declarations: &str, #[case] longhand: &str) {
        let mut f = Fixture::new();
        f.sheet(&format!(".a {{ {declarations} }}"));
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, longhand), None);
    }

    #[test]
    fn a_longhand_after_its_shorthand_wins_on_its_side() {
        let mut f = Fixture::new();
        f.sheet(".a { padding: 4px; padding-top: 8px }");
        let a = boxed(&mut f, "a");
        let spec = build_spec(&f.tree, a).unwrap();

        assert_eq!(spec.style.padding.top, Some(8.0));
        assert_eq!(spec.style.padding.left, Some(4.0));
    }

    #[test]
    fn shorthands_come_before_the_longhands_they_override() {
        // The reverse of the expected order.
        let props: Vec<(String, AttributeValue)> = [
            "width",
            "padding_top",
            "padding_x",
            "overflow",
            "padding",
            "margin",
        ]
        .map(|key| (key.to_owned(), num(1.0)))
        .into();

        let keys: Vec<&str> = shorthands_first(props.iter().map(|(key, value)| (key, value)))
            .into_iter()
            .map(|(key, _)| key.as_str())
            .collect();

        assert_eq!(
            keys,
            [
                "margin",
                "padding",
                "overflow",
                "padding_x",
                "padding_top",
                "width"
            ]
        );
    }

    #[test]
    fn an_invalid_declaration_keeps_the_earlier_value_of_its_key() {
        let mut f = Fixture::new();
        f.sheet(".a { width: 10px; width: bogus }");
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), Some(text("10px")));
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
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "height"), Some(text("2px")));
    }

    #[test]
    fn a_declaration_name_ignores_case() {
        let mut f = Fixture::new();
        f.sheet(".a { WIDTH: 1px }");
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), Some(text("1px")));
    }

    #[test]
    fn an_empty_value_is_dropped_silently() {
        let mut f = Fixture::new();
        f.sheet(".a { width: ; height: 2px }");
        let a = boxed(&mut f, "a");

        assert!(f.tree.take_style_warnings().is_empty());
        assert_eq!(value(&f, a, "width"), None);
    }

    #[rstest]
    #[case::around_the_value(".a { width: /* c */ 10px /* d */ }")]
    #[case::unterminated(".a { width: 10px /* c }")]
    #[case::before_important(".a { width: 10px /* c */ !important }")]
    fn a_comment_inside_a_value_is_skipped(#[case] css: &str) {
        let mut f = Fixture::new();
        f.sheet(css);
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), Some(text("10px")));
    }

    #[test]
    fn a_comment_before_a_selector_is_skipped() {
        let mut f = Fixture::new();
        f.sheet("/* c */ .a { width: 1px }");
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), Some(text("1px")));
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
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), Some(text("2px")));
    }

    #[test]
    fn an_at_rule_is_skipped_with_its_block() {
        let mut f = Fixture::new();
        f.sheet("@media (min-width: 1px) { .a { width: 1px } } .a { height: 2px }");
        let a = boxed(&mut f, "a");

        assert_eq!(
            f.tree.take_style_warnings(),
            ["ignoring the at-rule `@media`"]
        );
        assert_eq!(value(&f, a, "width"), None);
        assert_eq!(value(&f, a, "height"), Some(text("2px")));
    }

    #[test]
    fn important_applies_at_normal_importance() {
        let mut f = Fixture::new();
        f.sheet("#a { width: 5px !important } .a { width: 9px }");
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), Some(text("5px")));
    }

    #[test]
    fn a_style_element_applies_once_it_is_connected() {
        let mut f = Fixture::new();
        let (style, _) = f.sheet(".a { width: 1px }");
        let a = boxed(&mut f, "a");
        f.tree.remove_child(f.root, style).unwrap();
        let detached = value(&f, a, "width");
        f.tree.append_child(f.root, style).unwrap();

        assert_eq!(detached, None);
        assert_eq!(value(&f, a, "width"), Some(text("1px")));
    }

    #[test]
    fn editing_the_sheet_text_changes_the_next_read() {
        let mut f = Fixture::new();
        let (_, text_node) = f.sheet(".a { width: 1px }");
        let a = boxed(&mut f, "a");
        let before = value(&f, a, "width");
        f.attr(text_node, "value", ".a { width: 2px }");

        assert_eq!(before, Some(text("1px")));
        assert_eq!(value(&f, a, "width"), Some(text("2px")));
    }

    #[test]
    fn a_state_pseudo_class_matches_nothing_yet() {
        let mut f = Fixture::new();
        f.sheet(".a:hover { width: 1px }");
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), None);
    }

    #[test]
    fn a_sheet_warns_once_while_its_text_stays() {
        let mut f = Fixture::new();
        f.sheet(".a { bogus: 1 }");
        let a = boxed(&mut f, "a");
        let first = f.tree.take_style_warnings();
        f.tree.set_style(a, "width", 1.0).unwrap();

        assert_eq!(first, ["ignoring unknown style key `bogus`"]);
        assert!(f.tree.take_style_warnings().is_empty());
    }

    #[test]
    fn a_node_without_a_sheet_in_the_tree_borrows_its_inline_map() {
        let mut f = Fixture::new();
        let a = boxed(&mut f, "a");
        f.tree.set_style(a, "width", 1.0).unwrap();

        assert!(matches!(f.tree.computed_style(a), Cow::Borrowed(map) if map.len() == 1));
    }

    #[test]
    fn a_node_no_rule_styles_borrows_its_inline_map() {
        let mut f = Fixture::new();
        f.sheet(".a { width: 1px }");
        let b = boxed(&mut f, "b");

        assert!(matches!(f.tree.computed_style(b), Cow::Borrowed(_)));
    }

    #[test]
    fn a_style_tag_in_capitals_holds_rules() {
        let mut f = Fixture::new();
        let style = f.add(f.root, "STYLE", "sheet");
        let text_node = f.tree.create_node(TEXT_TAG).unwrap();
        f.attr(text_node, "value", ".a { width: 1px }");
        f.tree.append_child(style, text_node).unwrap();
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), Some(text("1px")));
    }

    #[test]
    fn a_comment_child_of_a_style_element_is_not_sheet_text() {
        let mut f = Fixture::new();
        let (style, _) = f.sheet(".a { height: 2px }");
        let comment = f.tree.create_node("#comment").unwrap();
        f.attr(comment, "value", ".a { width: 9px }");
        f.tree.append_child(style, comment).unwrap();
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), None);
        assert_eq!(value(&f, a, "height"), Some(text("2px")));
    }

    #[test]
    fn a_style_element_inside_a_style_element_holds_no_rules() {
        let mut f = Fixture::new();
        let (outer, _) = f.sheet("");
        let inner = f.tree.create_node("style").unwrap();
        let text_node = f.tree.create_node(TEXT_TAG).unwrap();
        f.attr(text_node, "value", ".a { width: 1px }");
        f.tree.append_child(inner, text_node).unwrap();
        f.tree.append_child(outer, inner).unwrap();
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), None);
    }

    #[test]
    fn sheets_apply_in_tree_order_not_creation_order() {
        let mut f = Fixture::new();
        let (first, _) = f.sheet(".a { width: 1px }");
        let (second, _) = f.sheet(".a { width: 2px }");
        f.tree.insert_before(f.root, second, Some(first)).unwrap();
        let a = boxed(&mut f, "a");

        assert_eq!(value(&f, a, "width"), Some(text("1px")));
    }
}
