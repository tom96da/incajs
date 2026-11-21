// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The checks on the names of an element a script creates.

/// The namespace the `xml` prefix is bound to.
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
/// The namespace of `xmlns` names.
const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";

/// Why a qualified name is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    /// A part of the name holds a character it must not.
    InvalidCharacter,
    /// The prefix does not fit the namespace.
    Namespace,
}

/// Splits `qualified_name` at its first colon into the prefix and the local name, which holds the
/// rest of the text, and checks both against `namespace`.
///
/// # Errors
///
/// `InvalidCharacter` when a part is no valid name, `Namespace` when the prefix does not fit
/// `namespace`.
pub fn validate_and_extract<'a>(
    namespace: Option<&str>,
    qualified_name: &'a str,
) -> Result<(Option<&'a str>, &'a str), NameError> {
    let (prefix, local_name) = match qualified_name.split_once(':') {
        Some((prefix, rest)) => (Some(prefix), rest),
        None => (None, qualified_name),
    };
    if !is_element_local_name(local_name) || prefix.is_some_and(|p| !is_prefix(p)) {
        return Err(NameError::InvalidCharacter);
    }
    let is_xmlns = qualified_name == "xmlns" || prefix == Some("xmlns");
    let allowed = match (prefix, namespace) {
        (Some(_), None) => false,
        (Some("xml"), namespace) if namespace != Some(XML_NS) => false,
        _ if is_xmlns => namespace == Some(XMLNS_NS),
        _ => namespace != Some(XMLNS_NS),
    };
    if allowed {
        Ok((prefix, local_name))
    } else {
        Err(NameError::Namespace)
    }
}

/// ASCII whitespace, NUL, `/` and `>`, which no part of an element name holds.
fn is_forbidden(c: char) -> bool {
    c.is_ascii_whitespace() || matches!(c, '\0' | '/' | '>')
}

/// A prefix is not empty and holds no forbidden character.
fn is_prefix(prefix: &str) -> bool {
    !prefix.is_empty() && !prefix.contains(is_forbidden)
}

/// An ASCII letter followed by no forbidden character, or a name that starts with `:`, `_` or a
/// code point from U+0080 on and continues with name characters.
pub fn is_element_local_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => !chars.any(is_forbidden),
        Some(':' | '_' | '\u{80}'..) => chars.all(is_name_char),
        _ => false,
    }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-' | '.') || c >= '\u{80}'
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::upper("DIV")]
    #[case::colon_first(":a")]
    #[case::underscore_first("_a")]
    #[case::colon_inside("a:b")]
    #[case::non_ascii("\u{c9}")]
    #[case::underscore_then_name_characters("_a-1.b")]
    #[case::underscore_then_a_non_xml_character("_\u{d7}")]
    #[case::parenthesis_after_a_letter("a(")]
    fn a_valid_local_name_is_accepted(#[case] name: &str) {
        assert!(is_element_local_name(name));
    }

    #[rstest]
    #[case::empty("")]
    #[case::hash("#document")]
    #[case::space("a b")]
    #[case::greater_than("a>")]
    #[case::slash("a/")]
    #[case::nul("a\0")]
    #[case::colon_first_then_a_space(":a b")]
    #[case::digit_first("1a")]
    #[case::dash_first("-b")]
    fn an_invalid_local_name_is_refused(#[case] name: &str) {
        assert!(!is_element_local_name(name));
    }

    #[rstest]
    #[case::second_colon_stays_in_the_name("a:b:c", "a", "b:c")]
    #[case::third_colon_stays_in_the_name("a:b:c:d", "a", "b:c:d")]
    #[case::trailing_colon("a:b:", "a", "b:")]
    #[case::double_colon("a::b", "a", ":b")]
    #[case::angle_in_local("a:b<", "a", "b<")]
    #[case::digit_prefix("1:a", "1", "a")]
    #[case::equals_sign_prefix("x=y:a", "x=y", "a")]
    #[case::non_ascii_prefix("\u{e9}:a", "\u{e9}", "a")]
    fn the_text_after_the_first_colon_is_the_local_name(
        #[case] name: &str,
        #[case] prefix: &str,
        #[case] local: &str,
    ) {
        assert_eq!(
            validate_and_extract(Some("urn:x"), name),
            Ok((Some(prefix), local))
        );
    }

    #[rstest]
    #[case::parenthesis("a(")]
    #[case::dot("a.b")]
    #[case::dash("a-b")]
    #[case::non_ascii("\u{c9}")]
    fn a_name_without_a_colon_has_no_prefix(#[case] name: &str) {
        assert_eq!(validate_and_extract(None, name), Ok((None, name)));
    }

    #[rstest]
    #[case::forbidden_after_a_second_colon("a:b:c/")]
    #[case::digit_local("a:1")]
    #[case::empty_prefix(":a")]
    #[case::empty_local("a:")]
    #[case::digit_first("1a")]
    #[case::space("a b")]
    #[case::empty("")]
    #[case::dash_first_local("a:-b")]
    #[case::space_in_prefix("a b:c")]
    #[case::angle_in_prefix("x>y:a")]
    #[case::slash_in_prefix("x/y:a")]
    #[case::nul_in_prefix("x\0y:a")]
    fn a_bad_name_is_an_invalid_character(#[case] name: &str) {
        assert_eq!(
            validate_and_extract(Some("urn:x"), name),
            Err(NameError::InvalidCharacter)
        );
    }

    #[rstest]
    #[case::prefix_without_namespace(None, "p:q")]
    #[case::digit_prefix_without_namespace(None, "1:a")]
    #[case::xmlns_name_without_namespace(None, "xmlns")]
    #[case::xml_prefix_in_another_namespace(Some("urn:x"), "xml:a")]
    #[case::xmlns_prefix_in_another_namespace(Some("urn:x"), "xmlns:a")]
    #[case::xmlns_name_in_another_namespace(Some("urn:x"), "xmlns")]
    #[case::xmlns_namespace_without_xmlns(Some(XMLNS_NS), "a")]
    fn a_prefix_that_does_not_fit_is_a_namespace_error(
        #[case] namespace: Option<&str>,
        #[case] name: &str,
    ) {
        assert_eq!(
            validate_and_extract(namespace, name),
            Err(NameError::Namespace)
        );
    }

    #[rstest]
    #[case::xml(Some(XML_NS), "xml:a")]
    #[case::xmlns(Some(XMLNS_NS), "xmlns:a")]
    #[case::xmlns_name(Some(XMLNS_NS), "xmlns")]
    fn the_reserved_prefix_fits_its_namespace(#[case] namespace: Option<&str>, #[case] name: &str) {
        assert!(validate_and_extract(namespace, name).is_ok());
    }
}
