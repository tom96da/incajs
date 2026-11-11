// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The token rules of the `class` attribute and the operations on the token set of a node.

use std::fmt;

use super::{AttributeValue, NodeId, TreeError, VirtualTree};

const CLASS: &str = "class";

/// How an operation on a class token list fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassListError {
    /// A token is empty.
    Empty,
    /// A token holds ASCII whitespace.
    Whitespace,
    /// The node is unknown.
    Tree(TreeError),
}

impl fmt::Display for ClassListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "the token must not be empty"),
            Self::Whitespace => write!(f, "the token must not contain whitespace"),
            Self::Tree(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ClassListError {}

impl From<TreeError> for ClassListError {
    fn from(error: TreeError) -> Self {
        Self::Tree(error)
    }
}

/// The tokens of `text`: split on ASCII whitespace, duplicates dropped, first occurrence kept.
pub(crate) fn parse_tokens(text: &str) -> Vec<&str> {
    let mut tokens: Vec<&str> = Vec::new();
    for token in text
        .split(|c: char| c.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
    {
        if !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    tokens
}

/// Checks the tokens in order. The first failure is returned.
pub(crate) fn check_tokens(tokens: &[&str]) -> Result<(), ClassListError> {
    for token in tokens {
        if token.is_empty() {
            return Err(ClassListError::Empty);
        }
        if token.contains(|c: char| c.is_ascii_whitespace()) {
            return Err(ClassListError::Whitespace);
        }
    }
    Ok(())
}

/// Checks the two tokens of `replace`: both for empty, then both for whitespace.
fn check_pair(token: &str, new_token: &str) -> Result<(), ClassListError> {
    if token.is_empty() || new_token.is_empty() {
        return Err(ClassListError::Empty);
    }
    check_tokens(&[token, new_token])
}

impl VirtualTree {
    /// The text of the `class` attribute, empty when it is absent.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn class_value(&self, id: NodeId) -> Result<String, TreeError> {
        let node = self.get(id).ok_or(TreeError::NodeNotFound(id))?;
        Ok(node
            .attributes()
            .get(CLASS)
            .map(AttributeValue::text)
            .unwrap_or_default())
    }

    /// The token set of the `class` attribute.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn class_tokens(&self, id: NodeId) -> Result<Vec<String>, TreeError> {
        let value = self.class_value(id)?;
        Ok(parse_tokens(&value)
            .into_iter()
            .map(str::to_owned)
            .collect())
    }

    /// Appends the tokens that are not in the set and writes the attribute, creating it.
    ///
    /// # Errors
    ///
    /// Returns [`ClassListError::Empty`] or [`ClassListError::Whitespace`] for the first bad
    /// token, and then changes nothing. Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn add_classes(&mut self, id: NodeId, tokens: &[&str]) -> Result<(), ClassListError> {
        check_tokens(tokens)?;
        let mut set = self.class_tokens(id)?;
        for token in tokens {
            if !set.iter().any(|t| t == token) {
                set.push((*token).to_owned());
            }
        }
        Ok(self.rewrite(id, &set)?)
    }

    /// Removes the tokens and writes the attribute. Nothing is written when the attribute is
    /// absent and the set is empty.
    ///
    /// # Errors
    ///
    /// Returns [`ClassListError::Empty`] or [`ClassListError::Whitespace`] for the first bad
    /// token. Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn remove_classes(&mut self, id: NodeId, tokens: &[&str]) -> Result<(), ClassListError> {
        check_tokens(tokens)?;
        let mut set = self.class_tokens(id)?;
        set.retain(|t| !tokens.contains(&t.as_str()));
        if !self.has_class_attribute(id) {
            return Ok(());
        }
        Ok(self.rewrite(id, &set)?)
    }

    /// Toggles one token. `force` of `Some(true)` only adds and `Some(false)` only removes.
    /// Returns whether the token is in the set afterwards. The attribute is written only when the
    /// set changes.
    ///
    /// # Errors
    ///
    /// Returns [`ClassListError::Empty`] or [`ClassListError::Whitespace`] for a bad token.
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn toggle_class(
        &mut self,
        id: NodeId,
        token: &str,
        force: Option<bool>,
    ) -> Result<bool, ClassListError> {
        check_tokens(&[token])?;
        let mut set = self.class_tokens(id)?;
        let present = set.iter().any(|t| t == token);
        if present {
            if force == Some(true) {
                return Ok(true);
            }
            set.retain(|t| t != token);
            self.rewrite(id, &set)?;
            return Ok(false);
        }
        if force == Some(false) {
            return Ok(false);
        }
        set.push(token.to_owned());
        self.rewrite(id, &set)?;
        Ok(true)
    }

    /// Replaces `token` with `new_token` at the first position of either, dropping every other
    /// occurrence of both. Returns false and changes nothing when `token` is not in the set.
    ///
    /// # Errors
    ///
    /// Returns [`ClassListError::Empty`] when either token is empty, otherwise
    /// [`ClassListError::Whitespace`] when either holds whitespace. Returns
    /// [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn replace_class(
        &mut self,
        id: NodeId,
        token: &str,
        new_token: &str,
    ) -> Result<bool, ClassListError> {
        check_pair(token, new_token)?;
        let set = self.class_tokens(id)?;
        let Some(first) = set.iter().position(|t| t == token || t == new_token) else {
            return Ok(false);
        };
        if !set.iter().any(|t| t == token) {
            return Ok(false);
        }
        let mut next = Vec::with_capacity(set.len());
        for (index, item) in set.into_iter().enumerate() {
            if index == first {
                next.push(new_token.to_owned());
            } else if item != token && item != new_token {
                next.push(item);
            }
        }
        self.rewrite(id, &next)?;
        Ok(true)
    }

    fn has_class_attribute(&self, id: NodeId) -> bool {
        self.get(id)
            .is_some_and(|node| node.attributes().contains_key(CLASS))
    }

    /// The update steps: the attribute becomes the set joined by one space. An existing attribute
    /// keeps its place in the attribute order.
    fn rewrite(&mut self, id: NodeId, set: &[String]) -> Result<(), TreeError> {
        self.set_attribute(id, CLASS, set.join(" "))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;

    /// A `div` whose `class` attribute is `class`, or absent for `None`.
    fn with_class(class: Option<&str>) -> (VirtualTree, NodeId) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_attribute(id, "title", "t").unwrap();
        if let Some(class) = class {
            tree.set_attribute(id, "class", class).unwrap();
        }
        tree.set_attribute(id, "lang", "en").unwrap();
        (tree, id)
    }

    fn class_attribute(tree: &VirtualTree, id: NodeId) -> Option<String> {
        tree.get(id)
            .unwrap()
            .attributes()
            .get("class")
            .map(AttributeValue::text)
    }

    #[rstest]
    #[case::space("a b", &["a", "b"])]
    #[case::tab("a\tb", &["a", "b"])]
    #[case::line_feed("a\nb", &["a", "b"])]
    #[case::form_feed("a\x0Cb", &["a", "b"])]
    #[case::carriage_return("a\rb", &["a", "b"])]
    #[case::vertical_tab_stays("a\x0Bb", &["a\x0Bb"])]
    #[case::no_break_space_stays("a\u{A0}b", &["a\u{A0}b"])]
    #[case::edges("  a  b  ", &["a", "b"])]
    #[case::duplicates("a b a c b", &["a", "b", "c"])]
    fn parse_tokens_splits_on_ascii_whitespace(#[case] text: &str, #[case] expected: &[&str]) {
        assert_eq!(parse_tokens(text), expected);
    }

    #[rstest]
    #[case::empty(&[""], ClassListError::Empty)]
    #[case::whitespace(&["a b"], ClassListError::Whitespace)]
    #[case::whitespace_first(&["a b", ""], ClassListError::Whitespace)]
    #[case::empty_first(&["", "a b"], ClassListError::Empty)]
    fn check_tokens_returns_the_first_failure(
        #[case] tokens: &[&str],
        #[case] expected: ClassListError,
    ) {
        assert_eq!(check_tokens(tokens), Err(expected));
    }

    #[rstest]
    #[case::empty_before_whitespace("a b", "", ClassListError::Empty)]
    #[case::whitespace_before_lookup("missing", "a b", ClassListError::Whitespace)]
    fn replace_class_checks_both_tokens_for_empty_first(
        #[case] token: &str,
        #[case] new_token: &str,
        #[case] expected: ClassListError,
    ) {
        let (mut tree, id) = with_class(Some("a"));

        assert_eq!(tree.replace_class(id, token, new_token), Err(expected));
    }

    #[rstest]
    #[case::add_empty(|t: &mut VirtualTree, id| t.add_classes(id, &[""]), ClassListError::Empty)]
    #[case::add_whitespace(|t: &mut VirtualTree, id| t.add_classes(id, &["a b"]), ClassListError::Whitespace)]
    #[case::remove_empty(|t: &mut VirtualTree, id| t.remove_classes(id, &[""]), ClassListError::Empty)]
    #[case::remove_whitespace(|t: &mut VirtualTree, id| t.remove_classes(id, &["a\tb"]), ClassListError::Whitespace)]
    #[case::toggle_empty(|t: &mut VirtualTree, id| t.toggle_class(id, "", None).map(drop), ClassListError::Empty)]
    #[case::toggle_whitespace(|t: &mut VirtualTree, id| t.toggle_class(id, "a b", None).map(drop), ClassListError::Whitespace)]
    fn operations_reject_a_bad_token(
        #[case] run: fn(&mut VirtualTree, NodeId) -> Result<(), ClassListError>,
        #[case] expected: ClassListError,
    ) {
        let (mut tree, id) = with_class(Some("a"));

        assert_eq!(run(&mut tree, id), Err(expected));
    }

    #[rstest]
    #[case::add_empty(|t: &mut VirtualTree, id| t.add_classes(id, &[""]))]
    #[case::add_whitespace(|t: &mut VirtualTree, id| t.add_classes(id, &["a b"]))]
    #[case::remove_empty(|t: &mut VirtualTree, id| t.remove_classes(id, &[""]))]
    #[case::remove_whitespace(|t: &mut VirtualTree, id| t.remove_classes(id, &["a\tb"]))]
    #[case::toggle_empty(|t: &mut VirtualTree, id| t.toggle_class(id, "", None).map(drop))]
    #[case::toggle_whitespace(|t: &mut VirtualTree, id| t.toggle_class(id, "a b", None).map(drop))]
    fn operations_leave_the_attribute_on_a_bad_token(
        #[case] run: fn(&mut VirtualTree, NodeId) -> Result<(), ClassListError>,
    ) {
        let (mut tree, id) = with_class(Some("a"));

        run(&mut tree, id).unwrap_err();

        assert_eq!(class_attribute(&tree, id).as_deref(), Some("a"));
    }

    #[test]
    fn add_changes_nothing_when_a_later_token_is_bad() {
        let (mut tree, id) = with_class(Some("a"));

        let result = tree.add_classes(id, &["b", "c d"]);

        assert_eq!(result, Err(ClassListError::Whitespace));
        assert_eq!(class_attribute(&tree, id).as_deref(), Some("a"));
    }

    #[test]
    fn add_appends_the_tokens_in_order() {
        let (mut tree, id) = with_class(Some("a b"));

        tree.add_classes(id, &["d", "c"]).unwrap();

        assert_eq!(class_attribute(&tree, id).as_deref(), Some("a b d c"));
    }

    #[test]
    fn add_skips_a_present_token() {
        let (mut tree, id) = with_class(Some("a b"));

        tree.add_classes(id, &["c", "a", "c"]).unwrap();

        assert_eq!(class_attribute(&tree, id).as_deref(), Some("a b c"));
    }

    #[test]
    fn add_creates_the_attribute_last_in_the_order() {
        let (mut tree, id) = with_class(None);

        tree.add_classes(id, &["a"]).unwrap();

        let keys: Vec<_> = tree.get(id).unwrap().attributes().keys().cloned().collect();
        assert_eq!(keys, ["title", "lang", "class"]);
    }

    #[test]
    fn rewrite_keeps_the_position_of_the_attribute() {
        let (mut tree, id) = with_class(Some("a"));

        tree.add_classes(id, &["b"]).unwrap();

        let keys: Vec<_> = tree.get(id).unwrap().attributes().keys().cloned().collect();
        assert_eq!(keys, ["title", "class", "lang"]);
    }

    #[test]
    fn remove_of_the_last_token_leaves_an_empty_attribute() {
        let (mut tree, id) = with_class(Some("a"));

        tree.remove_classes(id, &["a"]).unwrap();

        assert_eq!(class_attribute(&tree, id).as_deref(), Some(""));
    }

    #[rstest]
    #[case::remove_nothing(|t: &mut VirtualTree, id| t.remove_classes(id, &[]).map(drop))]
    #[case::remove_absent_token(|t: &mut VirtualTree, id| t.remove_classes(id, &["zz"]).map(drop))]
    #[case::toggle_off(|t: &mut VirtualTree, id| t.toggle_class(id, "zz", Some(false)).map(drop))]
    #[case::replace_absent_token(|t: &mut VirtualTree, id| t.replace_class(id, "zz", "y").map(drop))]
    fn operations_that_find_nothing_create_no_attribute(
        #[case] run: fn(&mut VirtualTree, NodeId) -> Result<(), ClassListError>,
    ) {
        let (mut tree, id) = with_class(None);

        run(&mut tree, id).unwrap();

        assert_eq!(class_attribute(&tree, id), None);
    }

    #[test]
    fn add_without_tokens_creates_the_attribute_empty() {
        let (mut tree, id) = with_class(None);

        tree.add_classes(id, &[]).unwrap();

        assert_eq!(class_attribute(&tree, id).as_deref(), Some(""));
    }

    #[test]
    fn add_without_tokens_normalizes_the_text() {
        let (mut tree, id) = with_class(Some("a  b"));

        tree.add_classes(id, &[]).unwrap();

        assert_eq!(class_attribute(&tree, id).as_deref(), Some("a b"));
    }

    #[test]
    fn remove_of_an_absent_token_normalizes_the_text() {
        let (mut tree, id) = with_class(Some("a  b"));

        tree.remove_classes(id, &["zz"]).unwrap();

        assert_eq!(class_attribute(&tree, id).as_deref(), Some("a b"));
    }

    #[rstest]
    #[case::remove_present("a b", None, "a", false)]
    #[case::force_false_removes("a b", Some(false), "a", false)]
    #[case::add_absent("a", None, "b", true)]
    #[case::force_true_adds("a", Some(true), "b", true)]
    #[case::force_true_on_present("a  b", Some(true), "a", true)]
    #[case::force_false_on_absent("a  b", Some(false), "c", false)]
    fn toggle_class_returns_the_membership(
        #[case] class: &str,
        #[case] force: Option<bool>,
        #[case] token: &str,
        #[case] expected: bool,
    ) {
        let (mut tree, id) = with_class(Some(class));

        assert_eq!(tree.toggle_class(id, token, force).unwrap(), expected);
    }

    #[rstest]
    #[case::remove_present("a b", None, "a", "b")]
    #[case::force_false_removes("a b", Some(false), "a", "b")]
    #[case::add_absent("a", None, "b", "a b")]
    #[case::force_true_adds("a", Some(true), "b", "a b")]
    #[case::force_true_on_present_keeps_the_text("a  b", Some(true), "a", "a  b")]
    #[case::force_false_on_absent_keeps_the_text("a  b", Some(false), "c", "a  b")]
    fn toggle_class_writes_the_text(
        #[case] class: &str,
        #[case] force: Option<bool>,
        #[case] token: &str,
        #[case] text: &str,
    ) {
        let (mut tree, id) = with_class(Some(class));

        tree.toggle_class(id, token, force).unwrap();

        assert_eq!(class_attribute(&tree, id).as_deref(), Some(text));
    }

    #[rstest]
    #[case::absent_token("a b", "zz", "y", false)]
    #[case::absent_token_with_present_new_token("a b", "x", "a", false)]
    #[case::new_token_takes_the_later_place("a b c", "a", "c", true)]
    #[case::new_token_takes_the_earlier_place("c b", "c", "a", true)]
    #[case::first_position_of_either("b x a", "a", "b", true)]
    #[case::same_token_keeps_the_set("a b", "a", "a", true)]
    fn replace_class_returns_whether_it_replaced(
        #[case] class: &str,
        #[case] token: &str,
        #[case] new_token: &str,
        #[case] expected: bool,
    ) {
        let (mut tree, id) = with_class(Some(class));

        assert_eq!(tree.replace_class(id, token, new_token).unwrap(), expected);
    }

    #[rstest]
    #[case::absent_token("a b", "zz", "y", "a b")]
    #[case::absent_token_with_present_new_token("a b", "x", "a", "a b")]
    #[case::absent_token_keeps_odd_spacing("a  b", "zz", "y", "a  b")]
    #[case::new_token_takes_the_later_place("a b c", "a", "c", "c b")]
    #[case::new_token_takes_the_earlier_place("c b", "c", "a", "a b")]
    #[case::first_position_of_either("b x a", "a", "b", "b x")]
    #[case::same_token_keeps_the_set("a b", "a", "a", "a b")]
    #[case::same_token_normalizes_the_text("a  b", "a", "a", "a b")]
    fn replace_class_writes_the_text(
        #[case] class: &str,
        #[case] token: &str,
        #[case] new_token: &str,
        #[case] text: &str,
    ) {
        let (mut tree, id) = with_class(Some(class));

        tree.replace_class(id, token, new_token).unwrap();

        assert_eq!(class_attribute(&tree, id).as_deref(), Some(text));
    }

    #[test]
    fn class_value_reads_a_number_as_its_text() {
        let (mut tree, id) = with_class(None);
        tree.set_attribute(id, "class", AttributeValue::Number(5.0))
            .unwrap();

        assert_eq!(tree.class_value(id).unwrap(), "5");
    }

    #[test]
    fn class_value_is_empty_when_the_attribute_is_absent() {
        let (tree, id) = with_class(None);

        assert_eq!(tree.class_value(id).unwrap(), "");
    }

    #[rstest]
    #[case::add_bad_token(|t: &mut VirtualTree| t.add_classes(999, &["a b"]).map(drop), ClassListError::Whitespace)]
    #[case::remove_bad_token(|t: &mut VirtualTree| t.remove_classes(999, &["a b"]), ClassListError::Whitespace)]
    #[case::toggle_bad_token(|t: &mut VirtualTree| t.toggle_class(999, "a b", None).map(drop), ClassListError::Whitespace)]
    #[case::replace_bad_token(|t: &mut VirtualTree| t.replace_class(999, "a b", "c").map(drop), ClassListError::Whitespace)]
    fn operations_check_tokens_before_the_lookup(
        #[case] run: fn(&mut VirtualTree) -> Result<(), ClassListError>,
        #[case] expected: ClassListError,
    ) {
        let (mut tree, _) = with_class(None);

        assert_eq!(run(&mut tree), Err(expected));
    }

    #[rstest]
    #[case::add(|t: &mut VirtualTree| t.add_classes(999, &["a"]).map(drop))]
    #[case::remove(|t: &mut VirtualTree| t.remove_classes(999, &["a"]))]
    #[case::toggle(|t: &mut VirtualTree| t.toggle_class(999, "a", None).map(drop))]
    #[case::replace(|t: &mut VirtualTree| t.replace_class(999, "a", "c").map(drop))]
    fn operations_report_an_unknown_node_for_good_tokens(
        #[case] run: fn(&mut VirtualTree) -> Result<(), ClassListError>,
    ) {
        let (mut tree, _) = with_class(None);

        assert_eq!(
            run(&mut tree),
            Err(ClassListError::Tree(TreeError::NodeNotFound(999)))
        );
    }
}
