// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::rc::Rc;

use rquickjs::{Ctx, Result as JsResult, Value};

use inca_document::ScrollRange;
use inca_document::geometry::Align;
use inca_node::NodeId;
use inca_style::is_scroll_container;

use super::Natives;
use super::convert::{JsAlign, JsNodeId, existing_node, or_null};
use super::host::Host;

pub(super) fn install_scroll(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("scrollOf", scroll_of)?;
    n.set("scrollNode", scroll_node)?;
    n.set("scrollIntoViewNode", scroll_into_view_node)
}

fn scroll_of<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id)| {
        existing_node(&ctx, &host.document.borrow().tree, node_id)?;
        let rect = node_scroll(&host, node_id);
        or_null(&ctx, rect.as_ref().map(<[f64; 4]>::as_slice))
    }
}

fn scroll_node<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, Option<f64>, Option<f64>) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), left, top| {
        existing_node(&ctx, &host.document.borrow().tree, node_id)?;
        scroll_to(&host, node_id, left, top);
        Ok(())
    }
}

fn scroll_into_view_node<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsAlign, JsAlign) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), JsAlign(block), JsAlign(inline)| {
        existing_node(&ctx, &host.document.borrow().tree, node_id)?;
        scroll_into_view(&host, node_id, block, inline);
        Ok(())
    }
}

/// The scroll offset of the node `id` and its maximum as of the last drawn frame: `left`, `top`,
/// `max left` and `max top`. A node that scrolls nothing has none.
pub(super) fn node_scroll(host: &Host, id: NodeId) -> Option<[f64; 4]> {
    let document = host.document.borrow();
    is_scroll_container(&document.tree, &document.style, id).then(|| {
        let ScrollRange { offset, max } = host.window.borrow().scroll_range(id);
        [offset.0, offset.1, max.0, max.1].map(f64::from)
    })
}

/// Moves the scroll offset of the node `id` on the given axes. A negative value counts as `0`.
pub(super) fn scroll_to(host: &Host, id: NodeId, left: Option<f64>, top: Option<f64>) {
    let document = host.document.borrow();
    if is_scroll_container(&document.tree, &document.style, id) {
        host.window
            .borrow()
            .scroll_to(&document.tree, id, left, top);
    }
}

/// Reveals the node `id` now, or at the next frame when it has no box yet.
pub(super) fn scroll_into_view(host: &Host, id: NodeId, block: Align, inline: Align) {
    if !host.reveal(id, block, inline) {
        host.window.borrow_mut().queue_reveal(id, block, inline);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::engine_with_bindings;

    #[test]
    fn scroll_of_and_scroll_node_raise_type_error_for_an_unknown_node() {
        let (engine, _host) = engine_with_bindings();

        let caught: Vec<bool> = engine
            .eval(
                r"
                const caught = (f) => { try { f(); } catch (e) { return e instanceof TypeError; } return false; };
                [caught(() => __inca_native__.scrollOf(999)), caught(() => __inca_native__.scrollNode(999, 1, 1))];
                ",
            )
            .unwrap();

        assert_eq!(caught, [true, true]);
    }

    #[test]
    fn scroll_into_view_node_throws_for_an_unknown_node_and_an_unknown_alignment() {
        let (engine, _host) = engine_with_bindings();

        let caught: Vec<bool> = engine
            .eval(
                r"
                const caught = (f) => { try { f(); } catch (e) { return e instanceof TypeError; } return false; };
                const n = __inca_native__;
                const node = n.createNode('div');
                [
                    caught(() => n.scrollIntoViewNode(999, 'start', 'start')),
                    caught(() => n.scrollIntoViewNode(node, 'middle', 'start')),
                    caught(() => n.scrollIntoViewNode(node, 'start', 'middle')),
                    caught(() => n.scrollIntoViewNode(node, 'start', 'nearest')),
                ];
                ",
            )
            .unwrap();

        assert_eq!(caught, [true, true, true, false]);
    }
}
