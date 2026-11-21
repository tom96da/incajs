// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The write of an attribute, with the `style` case.

use inca_node::{AttributeValue, NodeId, TreeError, VirtualTree};
use inca_style::set_style_text;

use crate::StyleWarnings;

/// Stores the attribute `name` of the node `node_id`. The `style` attribute writes its text as
/// style props and reports the lines it drops through `warnings`. Returns the folded key the
/// attribute has on the node.
///
/// # Errors
///
/// [`TreeError::NodeNotFound`] when `node_id` names no node.
pub fn store_attribute(
    tree: &mut VirtualTree,
    warnings: &mut StyleWarnings,
    node_id: NodeId,
    name: &str,
    value: AttributeValue,
) -> Result<String, TreeError> {
    let key = tree
        .get(node_id)
        .ok_or(TreeError::NodeNotFound(node_id))?
        .attribute_key(name)
        .into_owned();
    if key == "style" {
        for line in set_style_text(tree, node_id, &value.text())? {
            warnings.report(tree, node_id, &line);
        }
    } else {
        tree.set_attribute(node_id, key.clone(), value)?;
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    fn collecting() -> (StyleWarnings, Rc<RefCell<Vec<String>>>) {
        let lines = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&lines);
        let mut warnings = StyleWarnings::default();
        warnings.set_sink(Rc::new(move |line| sink.borrow_mut().push(line.to_owned())));
        (warnings, lines)
    }

    #[test]
    fn a_plain_attribute_is_stored_under_its_folded_key() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        let (mut warnings, _) = collecting();

        let key = store_attribute(&mut tree, &mut warnings, id, "TabIndex", "0".into()).unwrap();

        assert_eq!(key, "tabindex");
        assert_eq!(
            tree.get(id).unwrap().attributes().get("tabindex"),
            Some(&"0".into())
        );
    }

    #[test]
    fn a_style_attribute_warns_once_per_line() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        let (mut warnings, lines) = collecting();

        for _ in 0..2 {
            store_attribute(
                &mut tree,
                &mut warnings,
                id,
                "style",
                "bogus: 1; width: 2px".into(),
            )
            .unwrap();
        }

        assert_eq!(
            tree.get(id).unwrap().style_props().get("width"),
            Some(&"2px".into())
        );
        assert_eq!(lines.borrow().len(), 1, "{:?}", lines.borrow());
    }

    #[test]
    fn a_missing_node_is_an_error() {
        let mut tree = VirtualTree::new();
        let (mut warnings, lines) = collecting();

        let result = store_attribute(&mut tree, &mut warnings, 999, "style", "bogus: 1".into());

        assert_eq!(result, Err(TreeError::NodeNotFound(999)));
        assert!(lines.borrow().is_empty());
    }
}
