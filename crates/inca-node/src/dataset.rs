// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mapping between `data-*` attribute names and dataset keys.

use crate::tree::VirtualNode;

const PREFIX: &str = "data-";

/// A key that no `data-*` attribute name maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatasetError {
    /// The key holds a `-` followed by an ASCII lower case letter.
    Syntax,
}

/// The dataset key of an attribute name, or `None` when the name has no key.
///
/// The name starts with `data-` and holds no ASCII upper case letter. Each `-` that is followed by
/// an ASCII lower case letter is dropped, and that letter is upper cased.
#[must_use]
pub fn data_key(attribute: &str) -> Option<String> {
    let rest = attribute.strip_prefix(PREFIX)?;
    if rest.chars().any(|c| c.is_ascii_uppercase()) {
        return None;
    }
    let mut key = String::with_capacity(rest.len());
    let mut chars = rest.chars().peekable();
    while let Some(c) = chars.next() {
        match chars.peek() {
            Some(next) if c == '-' && next.is_ascii_lowercase() => {
                key.push(next.to_ascii_uppercase());
                chars.next();
            }
            _ => key.push(c),
        }
    }
    Some(key)
}

/// The `data-*` attribute name of a dataset key.
///
/// # Errors
///
/// Returns [`DatasetError::Syntax`] when the key holds a `-` followed by an ASCII lower case
/// letter.
pub fn data_attribute(key: &str) -> Result<String, DatasetError> {
    let mut name = String::with_capacity(PREFIX.len() + key.len());
    name.push_str(PREFIX);
    let mut chars = key.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '-' && chars.peek().is_some_and(char::is_ascii_lowercase) {
            return Err(DatasetError::Syntax);
        }
        if c.is_ascii_uppercase() {
            name.push('-');
            name.push(c.to_ascii_lowercase());
        } else {
            name.push(c);
        }
    }
    Ok(name)
}

impl VirtualNode {
    /// The dataset keys of the attributes that have one, in attribute order.
    #[must_use]
    pub fn dataset_keys(&self) -> Vec<String> {
        self.attributes()
            .keys()
            .filter_map(|name| data_key(name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tree::VirtualTree;
    use rstest::rstest;

    #[rstest]
    #[case::plain("data-foo", Some("foo"))]
    #[case::dash_before_lower("data-foo-bar", Some("fooBar"))]
    #[case::double_dash("data-foo--bar", Some("foo-Bar"))]
    #[case::leading_dash("data--x", Some("X"))]
    #[case::trailing_dash("data-x-", Some("x-"))]
    #[case::several_dashes("data-a-b-c", Some("aBC"))]
    #[case::dash_before_digit("data-foo-1", Some("foo-1"))]
    #[case::empty("data-", Some(""))]
    #[case::digit("data-1", Some("1"))]
    #[case::non_ascii("data-é", Some("é"))]
    #[case::upper_case_letter("data-Foo", None)]
    #[case::no_prefix("foo", None)]
    fn data_key_reads_each_attribute_name(#[case] attribute: &str, #[case] want: Option<&str>) {
        assert_eq!(data_key(attribute).as_deref(), want, "{attribute}");
    }

    #[rstest]
    #[case::camel("fooBar", Ok("data-foo-bar"))]
    #[case::upper_first("FooBar", Ok("data--foo-bar"))]
    #[case::dash_before_digit("foo-1", Ok("data-foo-1"))]
    #[case::empty("", Ok("data-"))]
    #[case::dash_before_lower("foo-bar", Err(DatasetError::Syntax))]
    #[case::leading_dash_before_lower("-x", Err(DatasetError::Syntax))]
    fn data_attribute_writes_each_key(#[case] key: &str, #[case] want: Result<&str, DatasetError>) {
        assert_eq!(
            data_attribute(key).as_deref().map_err(|e| *e),
            want,
            "{key}"
        );
    }

    #[test]
    fn dataset_keys_lists_the_keys_in_attribute_order() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        for name in ["data-z", "title", "data-a-x"] {
            tree.set_attribute(node, name, "1").unwrap();
        }

        assert_eq!(tree.get(node).unwrap().dataset_keys(), ["z", "aX"]);
    }

    #[test]
    fn dataset_keys_skips_an_attribute_with_an_upper_case_letter() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_attribute(node, "data-Foo", "1").unwrap();
        tree.set_attribute(node, "data-bar", "2").unwrap();

        assert_eq!(tree.get(node).unwrap().dataset_keys(), ["bar"]);
    }
}
