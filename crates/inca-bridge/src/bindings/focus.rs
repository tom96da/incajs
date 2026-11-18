// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use gpui::Bounds;
use rquickjs::{Ctx, Result as JsResult, Value};

use super::Natives;
use inca_node::NodeId;

use super::convert::{Flag, JsNodeId, existing_node, or_null};
use super::host::Host;

/// Requesting and releasing focus, and reading the focused node and the box of a node.
pub(super) fn install_focus(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("focusNode", focus_node)?;
    n.set("blurNode", blur_node)?;
    n.set("focusedNode", focused_node)?;
    n.set("layoutOf", layout_of)
}

fn focus_node<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId, Flag) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), prevent_scroll| {
        let mut host = host.borrow_mut();
        existing_node(&ctx, &host.tree, node_id)?;
        host.focus
            .request_focus(node_id, prevent_scroll.0.flatten() != Some(true));
        Ok(())
    }
}

fn blur_node(host: Rc<RefCell<Host>>) -> impl Fn(JsNodeId) {
    move |JsNodeId(node_id)| host.borrow_mut().focus.request_blur(node_id)
}

fn focused_node<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>) -> JsResult<Value<'js>> {
    move |ctx| {
        let host = host.borrow();
        or_null(&ctx, host.focus.active_node(&host.tree))
    }
}

/// The box of the node `id` in the last drawn frame: `x`, `y`, `width`, `height`, the border, the
/// scroll offset applied to the box and the two gutters. A node that is not connected has none,
/// whatever the frame record still holds, and an id the tree does not have is not connected.
pub(super) fn node_box(host: &Host, id: NodeId) -> Option<[f64; 9]> {
    if !host.tree.is_connected(id) {
        return None;
    }
    host.layout_of(id).map(|layout| {
        let Bounds { origin, size } = layout.bounds;
        [
            origin.x,
            origin.y,
            size.width,
            size.height,
            layout.border,
            layout.scroll.x,
            layout.scroll.y,
            layout.gutter.width,
            layout.gutter.height,
        ]
        .map(|px| f64::from(f32::from(px)))
    })
}

fn layout_of<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id)| {
        let rect = node_box(&host.borrow(), node_id);
        or_null(&ctx, rect.as_ref().map(<[f64; 9]>::as_slice))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::test_support::engine_with_bindings;
    use inca_gpui::NodeLayout;
    use inca_node::AttributeValue;

    #[test]
    fn only_focus_attributes_and_display_mark_a_node_for_a_focus_check() {
        let (engine, host) = engine_with_bindings();
        let node: u32 = engine
            .eval("globalThis.n = __inca_native__.createNode('div')")
            .unwrap();
        let dirty = |host: &Rc<RefCell<Host>>| host.borrow_mut().focus.take_tab_dirty();
        assert!(dirty(&host).is_empty());

        engine
            .eval::<()>(
                r"
                __inca_native__.setAttribute(n, 'label', 'x');
                __inca_native__.removeAttribute(n, 'label');
                __inca_native__.setStyle(n, 'width', 5);
                __inca_native__.removeStyle(n, 'width');
                ",
            )
            .unwrap();
        assert!(dirty(&host).is_empty());

        // A subtree check queues only nodes that have a tabindex.
        for call in [
            "setAttribute(n, 'tabindex', '0')",
            "setAttribute(n, 'tabIndex', 0)",
            "setAttribute(n, 'disabled', true)",
            "removeAttribute(n, 'disabled')",
            "setStyle(n, 'display', 'none')",
            "removeStyle(n, 'display')",
            "setAttribute(n, 'hidden', true)",
            "removeAttribute(n, 'hidden')",
            "setAttribute(n, 'inert', true)",
            "removeAttribute(n, 'inert')",
            "removeAttribute(n, 'TABINDEX')",
        ] {
            engine
                .eval::<()>(&format!("__inca_native__.{call}"))
                .unwrap();
            assert!(dirty(&host).contains(&node), "{call}");
        }
        engine
            .eval::<()>("__inca_native__.setAttribute(n, 'tabIndex', 1)")
            .unwrap();
        let host = host.borrow();
        let attrs = host.tree.get(node).unwrap().attributes();
        assert_eq!(attrs.get("tabindex"), Some(&AttributeValue::Number(1.0)));
        assert!(!attrs.contains_key("tabIndex"));
    }

    #[test]
    fn focused_node_follows_focus_node_and_blur_node() {
        let (engine, _host) = engine_with_bindings();

        let flags: Vec<bool> = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                const other = n.createNode('div');
                const plain = n.createNode('div');
                for (const id of [node, other, plain]) {
                    n.appendChild(n.rootNodeId(), id);
                }
                n.setAttribute(node, 'tabindex', 0);
                n.setAttribute(other, 'tabindex', 0);
                const before = n.focusedNode() === null;
                n.focusNode(plain);
                const plainSkipped = n.focusedNode() === null;
                n.focusNode(node);
                const focused = n.focusedNode() === node;
                n.blurNode(other);
                const kept = n.focusedNode() === node;
                n.blurNode(node);
                [before, plainSkipped, focused, kept, n.focusedNode() === null];
                ",
            )
            .unwrap();

        assert_eq!(flags, [true; 5]);
    }

    fn layout(x: f32) -> NodeLayout {
        NodeLayout {
            bounds: Bounds::new(
                gpui::point(gpui::px(x), gpui::px(2.0)),
                gpui::size(gpui::px(3.0), gpui::px(4.5)),
            ),
            border: gpui::px(1.0),
            scroll: gpui::point(gpui::px(6.0), gpui::px(7.5)),
            gutter: gpui::size(gpui::px(8.0), gpui::px(9.5)),
        }
    }

    #[test]
    fn layout_of_reads_the_recorded_box() {
        let (engine, host) = engine_with_bindings();
        let node: u32 = engine
            .eval(
                r"
                const id = __inca_native__.createNode('div');
                __inca_native__.appendChild(__inca_native__.rootNodeId(), id);
                id;
                ",
            )
            .unwrap();
        host.borrow().layout.borrow_mut().insert(node, layout(1.5));

        let read: Vec<f64> = engine
            .eval(&format!("__inca_native__.layoutOf({node})"))
            .unwrap();

        assert_eq!(read, [1.5, 2.0, 3.0, 4.5, 1.0, 6.0, 7.5, 8.0, 9.5]);
    }

    #[test]
    fn layout_of_a_node_with_no_record_is_null() {
        let (engine, _host) = engine_with_bindings();
        let node: u32 = engine.eval("__inca_native__.createNode('div')").unwrap();

        let is_null: bool = engine
            .eval(&format!("__inca_native__.layoutOf({node}) === null"))
            .unwrap();

        assert!(is_null);
    }

    #[test]
    fn layout_of_a_node_that_is_not_connected_is_null_whatever_the_record_holds() {
        let (engine, host) = engine_with_bindings();
        let node: u32 = engine
            .eval(
                r"
                const n = __inca_native__;
                globalThis.id = n.createNode('div');
                n.appendChild(n.rootNodeId(), id);
                n.removeChild(n.rootNodeId(), id);
                id;
                ",
            )
            .unwrap();
        host.borrow().layout.borrow_mut().insert(node, layout(1.5));

        let got: Vec<bool> = engine
            .eval("[__inca_native__.layoutOf(id) === null, __inca_native__.layoutOf(999) === null]")
            .unwrap();

        assert_eq!(got, [true, true]);
    }

    #[test]
    fn focus_node_on_an_unknown_node_throws_type_error() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.focusNode(999);
                } catch (e) {
                    caught = e instanceof TypeError;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught);
    }

    #[test]
    fn blur_node_ignores_an_unknown_node() {
        let (engine, _host) = engine_with_bindings();

        engine.eval::<()>("__inca_native__.blurNode(999);").unwrap();
    }
}
