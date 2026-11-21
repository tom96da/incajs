// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The inline style of a node as a block of declarations.

mod read;
mod write;

pub use read::{css_text, style_item, style_length, style_property_value};
pub use write::{
    remove_style, remove_style_property, set_style, set_style_property, set_style_text,
};

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tree::{AttributeValue, VirtualTree};

    pub(super) fn block(props: &[(&str, AttributeValue)]) -> (VirtualTree, crate::NodeId) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        for (key, value) in props {
            super::set_style(&mut tree, id, *key, value.clone()).unwrap();
        }
        (tree, id)
    }

    pub(super) fn entry(tree: &VirtualTree, id: crate::NodeId) -> Option<String> {
        tree.get(id)
            .unwrap()
            .attributes()
            .get("style")
            .map(AttributeValue::text)
    }

    pub(super) fn attribute_keys(tree: &VirtualTree, node: crate::NodeId) -> Vec<&str> {
        tree.get(node)
            .unwrap()
            .attributes()
            .keys()
            .map(String::as_str)
            .collect()
    }
}
