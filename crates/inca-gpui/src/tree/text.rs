// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The character data of text and comment nodes, kept in the `"value"` attribute.

use super::{AttributeValue, COMMENT_TAG, NodeId, TEXT_TAG, TreeError, VirtualNode, VirtualTree};

impl VirtualNode {
    /// The `"value"` attribute as text: empty when it is missing or not a string.
    pub(crate) fn data(&self) -> &str {
        self.attributes()
            .get("value")
            .and_then(AttributeValue::as_str)
            .unwrap_or_default()
    }
}

/// `data` as UTF-16 code units, the unit every offset counts.
fn units(data: &str) -> Vec<u16> {
    data.encode_utf16().collect()
}

/// The text of `units`. A half of a surrogate pair that stands alone becomes U+FFFD.
fn text_of(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

impl VirtualTree {
    fn data_of(&self, id: NodeId) -> Result<&str, TreeError> {
        self.get(id)
            .map(VirtualNode::data)
            .ok_or(TreeError::NodeNotFound(id))
    }

    /// The text a node contributes to `textContent`: the data of a text or comment node, and the
    /// data of every descendant text node in tree order for any other node.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn text_content(&self, id: NodeId) -> Result<String, TreeError> {
        let node = self.get(id).ok_or(TreeError::NodeNotFound(id))?;
        if matches!(node.tag_name(), TEXT_TAG | COMMENT_TAG) {
            return Ok(node.data().to_owned());
        }
        let mut text = String::new();
        let mut pending: Vec<NodeId> = node.children().iter().rev().copied().collect();
        while let Some(next) = pending.pop() {
            let Some(node) = self.get(next) else { continue };
            if node.tag_name() == TEXT_TAG {
                text.push_str(node.data());
            }
            pending.extend(node.children().iter().rev());
        }
        Ok(text)
    }

    /// The data of the text node `id` and of the text siblings that touch it, in order.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn whole_text(&self, id: NodeId) -> Result<String, TreeError> {
        let own = self.data_of(id)?;
        let is_text = |id: &NodeId| self.get(*id).is_some_and(|n| n.tag_name() == TEXT_TAG);
        let mut before = Vec::new();
        let mut at = id;
        while let Some(previous) = self.previous_sibling(at).filter(is_text) {
            before.push(previous);
            at = previous;
        }
        let mut text: String = before
            .iter()
            .rev()
            .filter_map(|&id| self.data_of(id).ok())
            .collect();
        text.push_str(own);
        let mut at = id;
        while let Some(next) = self.next_sibling(at).filter(is_text) {
            text.push_str(self.data_of(next)?);
            at = next;
        }
        Ok(text)
    }

    /// `count` code units of the data of `id` from `offset`, as many as the data holds.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node and [`TreeError::IndexSize`] if
    /// `offset` is beyond the length.
    pub fn substring_data(&self, id: NodeId, offset: u32, count: u32) -> Result<String, TreeError> {
        let data = units(self.data_of(id)?);
        let (start, end) = span(data.len(), offset, count)?;
        Ok(text_of(&data[start..end]))
    }

    /// Replaces `count` code units of the data of `id` from `offset` with `replacement`. A count
    /// past the end stops at the end.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node and [`TreeError::IndexSize`] if
    /// `offset` is beyond the length.
    pub fn replace_data(
        &mut self,
        id: NodeId,
        offset: u32,
        count: u32,
        replacement: &str,
    ) -> Result<(), TreeError> {
        let data = units(self.data_of(id)?);
        let (start, end) = span(data.len(), offset, count)?;
        let mut result = data[..start].to_vec();
        result.extend(replacement.encode_utf16());
        result.extend_from_slice(&data[end..]);
        self.set_attribute(id, "value", text_of(&result))
    }

    /// Cuts the data of `id` at `offset`: `id` keeps the head and a new text node holds the tail.
    /// The new node follows `id` when `id` has a parent and has none otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node and [`TreeError::IndexSize`] if
    /// `offset` is beyond the length.
    pub fn split_text(&mut self, id: NodeId, offset: u32) -> Result<NodeId, TreeError> {
        let data = units(self.data_of(id)?);
        let (cut, _) = span(data.len(), offset, 0)?;
        let tail = self.create_node(TEXT_TAG)?;
        self.set_attribute(tail, "value", text_of(&data[cut..]))?;
        self.set_attribute(id, "value", text_of(&data[..cut]))?;
        if let Some(parent) = self.get(id).and_then(VirtualNode::parent) {
            let anchor = self.next_sibling(id);
            self.insert_before(parent, tail, anchor)?;
        }
        Ok(tail)
    }
}

/// The range of `count` units from `offset` in data of `length` units, cut at the end.
fn span(length: usize, offset: u32, count: u32) -> Result<(usize, usize), TreeError> {
    let start = offset as usize;
    if start > length {
        return Err(TreeError::IndexSize);
    }
    Ok((start, length.min(start.saturating_add(count as usize))))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;

    fn leaf(tree: &mut VirtualTree, tag: &str, data: &str) -> NodeId {
        let id = tree.create_node(tag).unwrap();
        tree.set_attribute(id, "value", data).unwrap();
        id
    }

    fn data(tree: &VirtualTree, id: NodeId) -> String {
        tree.text_content(id).unwrap()
    }

    /// `outer` holds `a`, `inner` and `c`; `inner` holds a comment and `b`.
    fn nested() -> (VirtualTree, NodeId) {
        let mut tree = VirtualTree::new();
        let outer = tree.create_node("div").unwrap();
        let inner = tree.create_node("span").unwrap();
        let first = leaf(&mut tree, "#text", "a");
        let comment = leaf(&mut tree, "#comment", "x");
        let second = leaf(&mut tree, "#text", "b");
        let third = leaf(&mut tree, "#text", "c");
        tree.append_child(outer, first).unwrap();
        tree.append_child(outer, inner).unwrap();
        tree.append_child(outer, third).unwrap();
        tree.append_child(inner, comment).unwrap();
        tree.append_child(inner, second).unwrap();
        (tree, outer)
    }

    #[test]
    fn text_content_joins_descendant_text_in_tree_order() {
        let (tree, outer) = nested();

        assert!(data(&tree, outer).starts_with("ab"));
    }

    #[test]
    fn text_content_skips_comments() {
        let (tree, outer) = nested();

        assert_eq!(data(&tree, outer), "abc");
    }

    #[test]
    fn text_content_of_a_comment_is_its_data() {
        let mut tree = VirtualTree::new();
        let comment = leaf(&mut tree, "#comment", "note");

        assert_eq!(data(&tree, comment), "note");
    }

    #[rstest]
    #[case::append("abc", 3, 0, "d", "abcd")]
    #[case::insert("abc", 1, 0, "X", "aXbc")]
    #[case::delete("abc", 1, 1, "", "ac")]
    #[case::replace("abc", 1, 1, "XY", "aXYc")]
    #[case::count_past_the_end("abc", 1, 99, "", "a")]
    fn replace_data_edits_the_range(
        #[case] before: &str,
        #[case] offset: u32,
        #[case] count: u32,
        #[case] with: &str,
        #[case] after: &str,
    ) {
        let mut tree = VirtualTree::new();
        let id = leaf(&mut tree, "#text", before);

        tree.replace_data(id, offset, count, with).unwrap();

        assert_eq!(data(&tree, id), after);
    }

    #[rstest]
    #[case::substring_data(|tree: &mut VirtualTree, id: NodeId| tree.substring_data(id, 4, 1).unwrap_err())]
    #[case::replace_data(|tree: &mut VirtualTree, id: NodeId| tree.replace_data(id, 4, 0, "").unwrap_err())]
    #[case::split_text(|tree: &mut VirtualTree, id: NodeId| tree.split_text(id, 4).unwrap_err())]
    fn an_offset_above_the_length_is_index_size(
        #[case] call: fn(&mut VirtualTree, NodeId) -> TreeError,
    ) {
        let mut tree = VirtualTree::new();
        let id = leaf(&mut tree, "#text", "abc");

        assert_eq!(call(&mut tree, id), TreeError::IndexSize);
        assert_eq!(data(&tree, id), "abc");
    }

    #[test]
    fn an_offset_at_the_length_is_in_range() {
        let mut tree = VirtualTree::new();
        let id = leaf(&mut tree, "#text", "abc");

        assert_eq!(tree.substring_data(id, 3, 5).unwrap(), "");
    }

    #[test]
    fn offsets_count_utf16_units() {
        let mut tree = VirtualTree::new();
        let id = leaf(&mut tree, "#text", "a😀b");

        assert_eq!(tree.substring_data(id, 3, 1).unwrap(), "b");
    }

    #[test]
    fn an_offset_inside_a_surrogate_pair_leaves_replacement_characters() {
        let mut tree = VirtualTree::new();
        let id = leaf(&mut tree, "#text", "a😀b");

        let tail = tree.split_text(id, 2).unwrap();

        assert_eq!(data(&tree, id), "a\u{FFFD}");
        assert_eq!(data(&tree, tail), "\u{FFFD}b");
    }

    #[test]
    fn split_text_puts_the_new_node_right_after_the_old_one() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let old = leaf(&mut tree, "#text", "abcd");
        let last = leaf(&mut tree, "#text", "z");
        tree.append_child(parent, old).unwrap();
        tree.append_child(parent, last).unwrap();

        let tail = tree.split_text(old, 2).unwrap();

        assert_eq!(tree.get(parent).unwrap().children(), [old, tail, last]);
        assert_eq!(
            (data(&tree, old), data(&tree, tail)),
            ("ab".into(), "cd".into())
        );
    }

    #[test]
    fn split_text_leaves_the_new_node_parentless_without_a_parent() {
        let mut tree = VirtualTree::new();
        let old = leaf(&mut tree, "#text", "abcd");

        let tail = tree.split_text(old, 2).unwrap();

        assert_eq!(tree.get(tail).unwrap().parent(), None);
    }

    /// `parent` holds `x`, a `b` element, then `a`, `b` and `c`, another `b` element and `y`.
    fn walled() -> (VirtualTree, NodeId) {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let before = leaf(&mut tree, "#text", "x");
        let wall = tree.create_node("b").unwrap();
        let a = leaf(&mut tree, "#text", "a");
        let b = leaf(&mut tree, "#text", "b");
        let c = leaf(&mut tree, "#text", "c");
        let after = tree.create_node("b").unwrap();
        let beyond = leaf(&mut tree, "#text", "y");
        for child in [before, wall, a, b, c, after, beyond] {
            tree.append_child(parent, child).unwrap();
        }
        (tree, b)
    }

    #[test]
    fn whole_text_joins_the_touching_text_siblings_in_order() {
        let (tree, b) = walled();

        assert_eq!(tree.whole_text(b).unwrap(), "abc");
    }

    #[test]
    fn whole_text_stops_at_an_element_before_and_after() {
        let (tree, b) = walled();

        assert!(!tree.whole_text(b).unwrap().contains(['x', 'y']));
    }
}
