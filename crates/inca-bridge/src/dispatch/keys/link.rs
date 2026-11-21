// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The click that Enter makes on a focused link.

use gpui::{App, Window};
use inca_node::{NodeId, VirtualNode};

use crate::dispatch::EventDispatcher;

impl EventDispatcher {
    /// The focused node when it is a link. A link has no disabled state.
    pub(in crate::dispatch) fn focused_link(&self, window: &Window, cx: &App) -> Option<NodeId> {
        let host = self.host.document.borrow();
        host.focus
            .focused_node(window, cx)
            .filter(|id| host.tree.get(*id).is_some_and(VirtualNode::is_link))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::{HTML, MATHML, SVG};
    use crate::tests::keys::{Scene, scene};
    use gpui::TestAppContext;
    use inca_node::NodeId;
    use inca_test_macros::gpui_cases;

    impl Scene {
        /// A node of `namespace` named `tag` with `attributes`, before the field, recording its
        /// key events and its `click` into `out`.
        pub(in crate::dispatch::keys) fn add_link(
            &self,
            namespace: &str,
            tag: &str,
            attributes: &[(&str, &str)],
        ) -> NodeId {
            let node = {
                let mut host = self.host.document.borrow_mut();
                let node = host
                    .tree
                    .create_element_ns(Some(namespace), None, tag)
                    .unwrap();
                for &(key, value) in attributes {
                    host.tree.set_attribute(node, key, value).unwrap();
                }
                host.tree
                    .insert_before(self.parent, node, Some(self.field))
                    .unwrap();
                host.focus.mark_tab_dirty(node);
                node
            };
            for event in ["keydown", "keypress", "keyup", "click"] {
                self.engine
                    .eval::<()>(&format!(
                        r"
                        __inca_test_listen__({node}, '{event}', (e) => {{
                            if (e.type === 'click') {{
                                out.push(`click:${{e.button}}:${{e.detail}}:${{e.isTrusted}}:${{e.shiftKey}}`);
                            }} else {{
                                out.push(e.type);
                            }}
                            if (e.type === 'keydown' && globalThis.cancel) {{
                                e.preventDefault();
                            }}
                        }});
                        "
                    ))
                    .unwrap();
            }
            node
        }
    }

    const HREF: &[(&str, &str)] = &[("href", "u")];
    const CLICK: &str = "click:0:0:true:false";
    const ENTER: &[(&str, bool)] = &[("enter", false), ("enter", true)];

    #[gpui_cases]
    #[gpui::test]
    #[case::enter(HTML, HREF, ENTER, &["keydown", CLICK, "keyup"])]
    #[case::shift_enter(HTML, HREF, &[("shift-enter", false)], &["keydown", "click:0:0:true:true"])]
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
        for &(key, up) in keys {
            s.key(cx, key, up, false);
        }
        let events: Vec<String> = serde_json::from_str(&s.clicks()).unwrap();
        assert_eq!(events, want);
    }

    #[gpui::test]
    fn a_canceled_enter_key_down_clicks_no_link(cx: &mut TestAppContext) {
        let s = scene(cx);
        let link = s.add_link(HTML, "a", HREF);
        s.focus(cx, link);
        s.key(cx, "enter", false, true);
        assert_eq!(s.clicks(), r#"["keydown"]"#);
    }
}
