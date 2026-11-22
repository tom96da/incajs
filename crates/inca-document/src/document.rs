// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tree a document starts from.

use inca_node::VirtualTree;

/// A tree with a root element under a document node. The root is the node a mounting app
/// attaches under.
///
/// # Panics
///
/// Panics if a new tree has no free node id.
#[must_use]
pub fn new_document_tree() -> VirtualTree {
    let mut tree = VirtualTree::new();
    let root = tree
        .create_node("div")
        .expect("a new tree has free node ids");
    let document = tree
        .create_node("#document")
        .expect("a new tree has free node ids");
    tree.append_child(document, root)
        .expect("a new document takes its first element");
    tree.set_root(root);
    tree.set_document(document);
    tree
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn the_root_is_the_only_child_of_the_document() {
        let tree = new_document_tree();

        let (root, document) = (tree.root().unwrap(), tree.document().unwrap());

        assert_eq!(tree.get(document).unwrap().children(), [root]);
        assert_eq!(tree.get(root).unwrap().parent(), Some(document));
    }
}
