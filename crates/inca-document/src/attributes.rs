// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The writes of attributes and the class list, each with the focus re-check and the style warnings
//! it needs.

use inca_node::{AttributeValue, ClassListError, NodeId, TreeError};
use inca_style::set_style_text;

use crate::DocumentState;
use crate::semantics::AriaReferences;

impl DocumentState {
    /// Stores the attribute `name` of `node_id` under the key the node folds it to, and queues the
    /// focus re-check the key needs. The `style` attribute writes its text as style props and
    /// reports the lines it drops, once for each node and line.
    ///
    /// # Errors
    ///
    /// [`TreeError::NodeNotFound`] when `node_id` names no node.
    pub fn set_attribute(
        &mut self,
        node_id: NodeId,
        name: &str,
        value: impl Into<AttributeValue>,
    ) -> Result<(), TreeError> {
        let key = store_attribute(self, node_id, name, value.into())?;
        self.mark_attribute_dirty(node_id, &key);
        Ok(())
    }

    /// Removes the attribute `name` of `node_id`, folded as [`Self::set_attribute`] folds it, and
    /// queues the focus re-check the key needs.
    ///
    /// # Errors
    ///
    /// [`TreeError::NodeNotFound`] when `node_id` names no node.
    pub fn remove_attribute(&mut self, node_id: NodeId, name: &str) -> Result<(), TreeError> {
        let key = self
            .tree
            .get(node_id)
            .ok_or(TreeError::NodeNotFound(node_id))?
            .attribute_key(name)
            .into_owned();
        self.tree.remove_attribute(node_id, &key)?;
        self.mark_attribute_dirty(node_id, &key);
        Ok(())
    }

    /// Keeps `ids` as the elements the ARIA attribute `attribute` of `node_id` refers to, or
    /// removes the attribute for `None`. See [`AriaReferences::set_aria_elements`].
    ///
    /// # Errors
    ///
    /// [`TreeError::NodeNotFound`] when `node_id` names no node.
    pub fn set_aria_elements(
        &mut self,
        node_id: NodeId,
        attribute: &str,
        ids: Option<Vec<NodeId>>,
    ) -> Result<(), TreeError> {
        self.tree.set_aria_elements(node_id, attribute, ids)
    }

    /// Adds each of `tokens` that the class list of `node_id` lacks.
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::add_classes`](inca_node::VirtualTree::add_classes).
    pub fn add_classes(&mut self, node_id: NodeId, tokens: &[&str]) -> Result<(), ClassListError> {
        self.tree.add_classes(node_id, tokens)
    }

    /// Removes each of `tokens` from the class list of `node_id`.
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::remove_classes`](inca_node::VirtualTree::remove_classes).
    pub fn remove_classes(
        &mut self,
        node_id: NodeId,
        tokens: &[&str],
    ) -> Result<(), ClassListError> {
        self.tree.remove_classes(node_id, tokens)
    }

    /// Toggles `token` in the class list of `node_id`, or adds it for `force` true and removes it
    /// for `force` false. Returns whether the list holds the token after the call.
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::toggle_class`](inca_node::VirtualTree::toggle_class).
    pub fn toggle_class(
        &mut self,
        node_id: NodeId,
        token: &str,
        force: Option<bool>,
    ) -> Result<bool, ClassListError> {
        self.tree.toggle_class(node_id, token, force)
    }

    /// Replaces `token` with `new_token` in the class list of `node_id`. Returns whether the list
    /// held `token`.
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::replace_class`](inca_node::VirtualTree::replace_class).
    pub fn replace_class(
        &mut self,
        node_id: NodeId,
        token: &str,
        new_token: &str,
    ) -> Result<bool, ClassListError> {
        self.tree.replace_class(node_id, token, new_token)
    }
}

/// Stores the attribute `name` of the node `node_id`. The `style` attribute writes its text as
/// style props and reports the lines it drops. Returns the folded key the attribute has on the
/// node.
///
/// # Errors
///
/// [`TreeError::NodeNotFound`] when `node_id` names no node.
fn store_attribute(
    state: &mut DocumentState,
    node_id: NodeId,
    name: &str,
    value: AttributeValue,
) -> Result<String, TreeError> {
    let key = state
        .tree
        .get(node_id)
        .ok_or(TreeError::NodeNotFound(node_id))?
        .attribute_key(name)
        .into_owned();
    if key == "style" {
        for line in set_style_text(&mut state.tree, node_id, &value.text())? {
            state.warn_style(node_id, &line);
        }
    } else {
        state.tree.set_attribute(node_id, key.clone(), value)?;
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use inca_node::VirtualTree;

    use super::*;

    /// A document over a tree of one `div`, whose style warnings go to the returned list.
    fn state_with_div() -> (
        DocumentState,
        NodeId,
        std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        let mut state = DocumentState::from_tree(tree);
        let lines = state.record_warnings();
        (state, id, lines)
    }

    #[test]
    fn a_plain_attribute_is_stored_under_its_folded_key() {
        let (mut state, id, _) = state_with_div();

        state.set_attribute(id, "TabIndex", "0").unwrap();

        assert_eq!(
            state.tree().get(id).unwrap().attributes().get("tabindex"),
            Some(&"0".into())
        );
    }

    #[test]
    fn a_style_attribute_warns_once_per_line() {
        let (mut state, id, lines) = state_with_div();

        for _ in 0..2 {
            state
                .set_attribute(id, "style", "bogus: 1; width: 2px")
                .unwrap();
        }

        assert_eq!(
            state.tree().get(id).unwrap().style_props().get("width"),
            Some(&"2px".into())
        );
        assert_eq!(lines.borrow().len(), 1, "{:?}", lines.borrow());
    }

    #[test]
    fn a_missing_node_is_an_error() {
        let (mut state, _, lines) = state_with_div();

        let result = state.set_attribute(999, "style", "bogus: 1");

        assert_eq!(result, Err(TreeError::NodeNotFound(999)));
        assert!(lines.borrow().is_empty());
    }
}
