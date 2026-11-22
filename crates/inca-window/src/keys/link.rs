// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The click that Enter makes on a focused link.

use gpui::{App, Window};
use inca_node::{NodeId, VirtualNode};

use crate::dispatch::EventDispatcher;

impl EventDispatcher {
    /// The focused node when it is a link. A link has no disabled state.
    pub(crate) fn focused_link(&self, window: &Window, cx: &App) -> Option<NodeId> {
        let document = self.cells.document.borrow();
        self.cells
            .window
            .borrow()
            .focus
            .focused_node(&document.focus, window, cx)
            .filter(|id| document.tree.get(*id).is_some_and(VirtualNode::is_link))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tests::keys::{Scene, scene};
    use crate::tests::{HTML, MATHML, SVG};
    use gpui::TestAppContext;
    use inca_node::NodeId;
    use inca_test_macros::gpui_cases;

    impl Scene {
        /// A node of `namespace` named `tag` with `attributes`, before the field.
        pub(crate) fn add_link(
            &self,
            namespace: &str,
            tag: &str,
            attributes: &[(&str, &str)],
        ) -> NodeId {
            let mut document = self.cells.document.borrow_mut();
            let node = document
                .tree
                .create_element_ns(Some(namespace), None, tag)
                .unwrap();
            for &(key, value) in attributes {
                document.tree.set_attribute(node, key, value).unwrap();
            }
            document
                .tree
                .insert_before(self.parent, node, Some(self.field))
                .unwrap();
            document.focus.mark_tab_dirty(node);
            node
        }

        /// The key events at `node`, and its clicks as `click:button:detail:shiftKey`.
        fn events_at(&self, node: NodeId) -> Vec<String> {
            self.walks()
                .into_iter()
                .filter(|walked| walked.target == inca_document::EventTargetId::Node(node))
                .filter_map(|walked| match walked.name.as_str() {
                    "click" => {
                        let click = walked.mouse();
                        Some(format!(
                            "click:{}:{}:{}",
                            click.button,
                            click.detail,
                            click.modifiers.shift()
                        ))
                    }
                    "keydown" | "keypress" | "keyup" => Some(walked.name),
                    _ => None,
                })
                .collect()
        }
    }

    const HREF: &[(&str, &str)] = &[("href", "u")];
    const CLICK: &str = "click:0:0:false";
    const ENTER: &[(&str, bool)] = &[("enter", false), ("enter", true)];

    #[gpui_cases]
    #[gpui::test]
    #[case::enter(HTML, HREF, ENTER, &["keydown", CLICK, "keyup"])]
    #[case::shift_enter(HTML, HREF, &[("shift-enter", false)], &["keydown", "click:0:0:true"])]
    #[case::svg_href(SVG, HREF, ENTER, &["keydown", CLICK, "keyup"])]
    #[case::svg_xlink_href(SVG, &[("xlink:href", "u")], ENTER, &["keydown", CLICK, "keyup"])]
    #[case::disabled(HTML, &[("href", "u"), ("disabled", "")], ENTER, &["keydown", CLICK, "keyup"])]
    #[case::space(HTML, HREF, &[("space-> ", false), ("space", true)], &["keydown", "keypress", "keyup"])]
    #[case::space_enter_space(
        HTML, HREF,
        &[("space-> ", false), ("enter", false), ("space", true)],
        &["keydown", "keypress", "keydown", CLICK, "keyup"]
    )]
    #[case::no_href(HTML, &[("tabindex", "0")], ENTER, &["keydown", "keypress", "keyup"])]
    #[case::mathml_href(MATHML, &[("href", "u"), ("tabindex", "0")], ENTER, &["keydown", "keypress", "keyup"])]
    fn a_focused_link_clicks_on_enter_only(
        cx: &mut TestAppContext,
        namespace: &str,
        attributes: &[(&str, &str)],
        keys: &[(&str, bool)],
        want: &[&str],
    ) {
        let s = scene(cx);
        let link = s.add_link(namespace, "a", attributes);
        s.focus(cx, link);
        s.forget();
        for &(key, up) in keys {
            s.key(cx, key, up, false);
        }
        assert_eq!(s.events_at(link), want);
    }

    #[gpui::test]
    fn a_canceled_enter_key_down_clicks_no_link(cx: &mut TestAppContext) {
        let s = scene(cx);
        let link = s.add_link(HTML, "a", HREF);
        s.focus(cx, link);
        s.forget();
        s.key(cx, "enter", false, true);
        assert_eq!(s.events_at(link), ["keydown"]);
    }
}
