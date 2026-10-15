// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use gpui::Bounds;
use rquickjs::function::Opt;
use rquickjs::{Ctx, Result as JsResult, Value};

use super::Natives;
use super::convert::{JsNodeId, existing_node, or_null};
use super::host::Host;

/// Requesting and releasing focus.
pub(super) fn install_focus(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("focusNode", focus_node)?;
    n.set("blurNode", blur_node)?;
    n.set("focusedNode", focused_node)?;
    n.set("layoutOf", layout_of)
}

fn focus_node(host: Rc<RefCell<Host>>) -> impl Fn(JsNodeId, Opt<bool>) {
    move |JsNodeId(node_id), prevent_scroll| {
        let mut host = host.borrow_mut();
        // Focusing a node that is gone is a no-op.
        if host.tree.get(node_id).is_some() {
            host.focus
                .request_focus(node_id, prevent_scroll.0 != Some(true));
        }
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

fn layout_of<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        let layout = host.layout.borrow().get(&node_id).copied();
        let rect = layout.map(|layout| {
            let Bounds { origin, size } = layout.bounds;
            [
                origin.x,
                origin.y,
                size.width,
                size.height,
                layout.border,
                layout.scroll.x,
                layout.scroll.y,
            ]
            .map(|px| f64::from(f32::from(px)))
        });
        or_null(&ctx, rect.as_ref().map(<[f64; 7]>::as_slice))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::test_support::engine_with_bindings;
    use inca_gpui::{AttributeValue, NodeLayout};

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
        }
    }

    #[test]
    fn layout_of_reads_the_recorded_box() {
        let (engine, host) = engine_with_bindings();
        let node: u32 = engine.eval("__inca_native__.createNode('div')").unwrap();
        host.borrow().layout.borrow_mut().insert(node, layout(1.5));

        let read: Vec<f64> = engine
            .eval(&format!("__inca_native__.layoutOf({node})"))
            .unwrap();

        assert_eq!(read, [1.5, 2.0, 3.0, 4.5, 1.0, 6.0, 7.5]);
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
    fn layout_of_an_unknown_node_throws_type_error() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try { __inca_native__.layoutOf(999); } catch (e) { caught = e instanceof TypeError; }
                caught;
                ",
            )
            .unwrap();

        assert!(caught);
    }
}
