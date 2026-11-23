// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The operations of the document on the pointer capture request that waits for the next pointer
//! event.

use inca_node::NodeId;

use crate::{DocumentState, View};

impl DocumentState {
    /// Asks for the capture of the mouse pointer for `node`, which starts at the next pointer
    /// event. The request is kept only while a button of `view` is held and `node` is connected.
    /// Returns whether `node` is connected.
    #[must_use]
    pub fn request_capture(&mut self, view: &dyn View, node: NodeId) -> bool {
        let connected = self.tree.is_connected(node);
        if connected && view.held_buttons() != 0 {
            self.capture_request = Some(node);
        }
        connected
    }

    /// Drops the capture request when it names `node`.
    pub fn release_capture_request(&mut self, node: NodeId) {
        if self.capture_request == Some(node) {
            self.capture_request = None;
        }
    }

    /// Whether the capture request names `node` and `node` is connected.
    #[must_use]
    pub fn has_capture_request(&self, node: NodeId) -> bool {
        self.capture_request == Some(node) && self.tree.is_connected(node)
    }

    /// The node the capture request names, for the window to apply. A request whose node is not
    /// connected is dropped and returns `None`.
    pub fn capture_request(&mut self) -> Option<NodeId> {
        let tree = &self.tree;
        self.capture_request = self.capture_request.filter(|&node| tree.is_connected(node));
        self.capture_request
    }

    /// Drops the capture request.
    pub fn cancel_capture_request(&mut self) {
        self.capture_request = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::tests::mounted;

    #[rstest]
    #[case::held(1, true, (true, true))]
    #[case::no_button(0, true, (true, false))]
    #[case::detached(1, false, (false, false))]
    fn a_capture_request_is_kept_for_a_connected_node_while_a_button_is_held(
        #[case] held: u8,
        #[case] attached: bool,
        #[case] expected: (bool, bool),
    ) {
        let (mut document, mut view, mut node) = mounted();
        view.held = held;
        if !attached {
            node = document.create_node("div").unwrap();
        }

        let connected = document.request_capture(&view, node);

        assert_eq!((connected, document.has_capture_request(node)), expected);
    }

    #[test]
    fn a_capture_release_drops_only_the_request_of_its_node() {
        let (mut document, view, node) = mounted();
        let other = document.create_node("div").unwrap();
        document
            .append_child(&view, document.root(), other)
            .unwrap();
        assert!(document.request_capture(&view, node));

        document.release_capture_request(other);
        let kept = document.has_capture_request(node);
        document.release_capture_request(node);

        assert_eq!((kept, document.has_capture_request(node)), (true, false));
    }

    #[test]
    fn a_capture_request_of_a_node_that_left_the_tree_is_dropped() {
        let (mut document, view, node) = mounted();
        assert!(document.request_capture(&view, node));
        let root = document.root();
        document.remove_child(&view, root, node).unwrap();
        let removed = (
            document.has_capture_request(node),
            document.capture_request(),
        );

        document.append_child(&view, root, node).unwrap();

        assert_eq!(removed, (false, None));
        assert_eq!(document.capture_request(), None);
    }
}
