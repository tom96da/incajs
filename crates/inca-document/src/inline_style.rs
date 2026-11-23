// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The writes of the inline style, each with the focus re-check and the style warnings it needs.

use inca_node::{AttributeValue, NodeId, TreeError};
use inca_style::{
    remove_style, remove_style_property, set_style, set_style_property, set_style_text,
    style_warning,
};

use crate::DocumentState;

impl DocumentState {
    /// Stores the style prop `key` of `node_id`, queues the focus re-check the key needs and
    /// reports a key or value the document does not support, once for each node and line.
    ///
    /// # Errors
    ///
    /// [`TreeError::NodeNotFound`] when `node_id` names no node.
    pub fn set_style_prop(
        &mut self,
        node_id: NodeId,
        key: &str,
        value: impl Into<AttributeValue>,
    ) -> Result<(), TreeError> {
        let value = value.into();
        let warning = style_warning(key, &value);
        set_style(&mut self.tree, node_id, key, value)?;
        self.mark_style_dirty(node_id, key);
        if let Some(warning) = warning {
            self.warn_style(node_id, &warning);
        }
        Ok(())
    }

    /// Removes the style prop `key` of `node_id` and queues the focus re-check the key needs.
    ///
    /// # Errors
    ///
    /// [`TreeError::NodeNotFound`] when `node_id` names no node.
    pub fn remove_style_prop(&mut self, node_id: NodeId, key: &str) -> Result<(), TreeError> {
        remove_style(&mut self.tree, node_id, key)?;
        self.mark_style_dirty(node_id, key);
        Ok(())
    }

    /// Replaces the inline style of `node_id` with the declarations of `text`, reports the lines
    /// it drops and queues a focus re-check of the subtree.
    ///
    /// # Errors
    ///
    /// [`TreeError::NodeNotFound`] when `node_id` names no node.
    pub fn set_style_text(&mut self, node_id: NodeId, text: &str) -> Result<(), TreeError> {
        let lines = set_style_text(&mut self.tree, node_id, text)?;
        for line in &lines {
            self.warn_style(node_id, line);
        }
        self.mark_focus_dirty(node_id);
        Ok(())
    }

    /// Writes the CSS property `name` of the inline style of `node_id` with the text `value`,
    /// reports the lines it drops and queues the focus re-check the property needs.
    ///
    /// # Errors
    ///
    /// [`TreeError::NodeNotFound`] when `node_id` names no node.
    pub fn set_style_property(
        &mut self,
        node_id: NodeId,
        name: &str,
        value: &str,
    ) -> Result<(), TreeError> {
        let lines = set_style_property(&mut self.tree, node_id, name, value)?;
        for line in &lines {
            self.warn_style(node_id, line);
        }
        self.mark_property_dirty(node_id, name);
        Ok(())
    }

    /// Removes the CSS property `name` of the inline style of `node_id` and queues the focus
    /// re-check the property needs. Returns the text the property had.
    ///
    /// # Errors
    ///
    /// [`TreeError::NodeNotFound`] when `node_id` names no node.
    pub fn remove_style_property(
        &mut self,
        node_id: NodeId,
        name: &str,
    ) -> Result<String, TreeError> {
        let former = remove_style_property(&mut self.tree, node_id, name)?;
        self.mark_property_dirty(node_id, name);
        Ok(former)
    }
}
