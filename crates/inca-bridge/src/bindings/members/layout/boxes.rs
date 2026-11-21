// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reads of the box of an element: the rect, the client and offset reads. A node that is not
//! connected has no box and reads `0`.

use std::rc::Rc;

use rquickjs::{Ctx, Object, Result as JsResult};

use inca_jsenv::geometry::dom_rect;
use inca_node::{Link, NodeId};

use super::super::{Shared, getter, method};
use crate::bindings::focus::node_box;
use crate::bindings::handle::{ELEMENT, HTML_ELEMENT};
use crate::bindings::host::Host;

/// `Math.round` of JS, with `-0` and `NaN` read as `0`.
pub(super) fn rounded(value: f64) -> f64 {
    let rounded = (value + 0.5).floor();
    if rounded.is_nan() || rounded == 0.0 {
        0.0
    } else {
        rounded
    }
}

/// The box of the node `id`, `None` when it has none.
pub(super) fn box_of(host: &Shared, id: NodeId) -> Option<[f64; 9]> {
    node_box(host, id)
}

pub(super) fn client_width(host: &Shared, id: NodeId) -> f64 {
    box_of(host, id).map_or(0.0, |b| rounded(b[2] - 2.0 * b[4] - b[7]).max(0.0))
}

pub(super) fn client_height(host: &Shared, id: NodeId) -> f64 {
    box_of(host, id).map_or(0.0, |b| rounded(b[3] - 2.0 * b[4] - b[8]).max(0.0))
}

/// Where the node `id` sits inside the padding edge of its offset parent, rounded. The offset
/// parent is the parent element when it has a box too. A node with no box gives `0` and `0`.
fn offset_of(host: &Shared, id: NodeId) -> (f64, f64) {
    let Some(b) = box_of(host, id) else {
        return (0.0, 0.0);
    };
    let parent = host.document.borrow().tree.linked(id, Link::Parent, true);
    let origin = parent.and_then(|parent| box_of(host, parent));
    let (ox, oy) = origin.map_or((0.0, 0.0), |o| (o[0] + o[4], o[1] + o[4]));
    // The box moved with the scroll its parent applied when it was recorded.
    (rounded(b[0] - ox + b[5]), rounded(b[1] - oy + b[6]))
}

/// Defines the rect and the client reads on `proto`, the prototype of `Element`.
pub(super) fn install_client<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Rc<Host>,
) -> JsResult<()> {
    method(
        ctx,
        proto,
        host,
        "getBoundingClientRect",
        0,
        &ELEMENT,
        |ctx, host, id, _, _| {
            let [x, y, width, height, ..] = box_of(host, id).unwrap_or([0.0; 9]);
            dom_rect(ctx, x, y, width, height)
        },
    )?;
    getter(
        ctx,
        proto,
        host,
        "clientWidth",
        &ELEMENT,
        |_, host, id, _| Ok(client_width(host, id)),
    )?;
    getter(
        ctx,
        proto,
        host,
        "clientHeight",
        &ELEMENT,
        |_, host, id, _| Ok(client_height(host, id)),
    )?;
    for name in ["clientLeft", "clientTop"] {
        getter(ctx, proto, host, name, &ELEMENT, |_, host, id, _| {
            Ok(box_of(host, id).map_or(0.0, |b| rounded(b[4])))
        })?;
    }
    Ok(())
}

/// Defines the offset reads on `proto`, the prototype of `HTMLElement`.
pub(super) fn install_offset<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Rc<Host>,
) -> JsResult<()> {
    getter(
        ctx,
        proto,
        host,
        "offsetWidth",
        &HTML_ELEMENT,
        |_, host, id, _| Ok(box_of(host, id).map_or(0.0, |b| rounded(b[2]))),
    )?;
    getter(
        ctx,
        proto,
        host,
        "offsetHeight",
        &HTML_ELEMENT,
        |_, host, id, _| Ok(box_of(host, id).map_or(0.0, |b| rounded(b[3]))),
    )?;
    getter(
        ctx,
        proto,
        host,
        "offsetLeft",
        &HTML_ELEMENT,
        |_, host, id, _| Ok(offset_of(host, id).0),
    )?;
    getter(
        ctx,
        proto,
        host,
        "offsetTop",
        &HTML_ELEMENT,
        |_, host, id, _| Ok(offset_of(host, id).1),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use super::super::fixture::{read, record, with_box};
    use crate::test_support::engine_with_bindings;

    #[rstest]
    #[case::client_width("el.clientWidth", 11)]
    #[case::client_height("el.clientHeight", 23)]
    #[case::client_left("el.clientLeft", 2)]
    #[case::client_top("el.clientTop", 2)]
    #[case::offset_width("el.offsetWidth", 20)]
    #[case::offset_height("el.offsetHeight", 30)]
    #[case::offset_left("el.offsetLeft", 14)]
    #[case::offset_top("el.offsetTop", 5)]
    #[case::scroll_width("el.scrollWidth", 11)]
    #[case::scroll_height("el.scrollHeight", 23)]
    fn a_layout_read_computes_from_the_recorded_box(#[case] expr: &str, #[case] expected: i64) {
        // width 20, height 30, border 2, gutter 5 and 3, scroll (4, 5), at (10, 0). The maximum
        // scroll offset is zero in a unit test.
        let (engine, _host, _id) = with_box([10.0, 0.0, 20.0, 30.0, 2.0, 4.0, 5.0, 5.0, 3.0]);

        assert_eq!(read(&engine, expr), json!(expected));
    }

    #[test]
    fn an_offset_starts_at_the_padding_edge_of_a_parent_with_a_box() {
        let (engine, host, _id) = with_box([100.0, 50.0, 200.0, 200.0, 3.0, 0.0, 0.0, 0.0, 0.0]);
        let child: u32 = engine
            .eval(
                r"
                const child = n.createNode('div');
                n.appendChild(id, child);
                globalThis.el = n.nodeHandle(child)[0];
                child;
                ",
            )
            .unwrap();
        record(
            &host,
            child,
            [110.0, 60.0, 20.0, 20.0, 0.0, 4.0, 5.0, 0.0, 0.0],
        );

        assert_eq!(
            read(&engine, "[el.offsetLeft, el.offsetTop]"),
            json!([11, 12])
        );
    }

    #[test]
    fn a_layout_read_of_a_node_removed_before_the_frame_is_zero() {
        let (engine, _host, _id) = with_box([1.0, 2.0, 30.0, 40.0, 1.0, 0.0, 0.0, 0.0, 0.0]);

        let got = read(
            &engine,
            r"
            (() => {
                const live = el.offsetWidth;
                __inca_native__.removeChild(__inca_native__.rootNodeId(), id);
                return [
                    live,
                    el.offsetWidth,
                    el.clientWidth,
                    el.scrollTop,
                    el.scrollWidth,
                    el.getBoundingClientRect().width,
                ];
            })()
            ",
        );

        assert_eq!(got, json!([30, 0, 0, 0, 0, 0]));
    }

    #[test]
    fn a_freed_node_reads_zero() {
        let (engine, _host, _id) = with_box([1.0, 2.0, 30.0, 40.0, 1.0, 0.0, 0.0, 0.0, 0.0]);

        let got = read(
            &engine,
            r"
            (() => {
                __inca_native__.destroyNode(id);
                return [el.offsetWidth, el.offsetLeft, el.scrollLeft, el.clientTop];
            })()
            ",
        );

        assert_eq!(got, json!([0, 0, 0, 0]));
    }

    #[test]
    fn the_bounding_rect_is_a_new_unrounded_dom_rect_per_call() {
        let (engine, _host, _id) = with_box([1.5, 2.25, 10.5, 4.75, 0.0, 0.0, 0.0, 0.0, 0.0]);

        let got = read(
            &engine,
            r"
            (() => {
                const a = el.getBoundingClientRect();
                const b = el.getBoundingClientRect();
                return [a instanceof DOMRect, Object.keys(a).length, a.x, a.y, a.width, a.height, a === b];
            })()
            ",
        );

        assert_eq!(got, json!([true, 0, 1.5, 2.25, 10.5, 4.75, false]));
    }

    #[test]
    fn the_bounding_rect_of_a_node_with_no_box_is_zeros() {
        let (engine, _host) = engine_with_bindings();

        let got = read(
            &engine,
            r"
            (() => {
                const e = __inca_native__.nodeHandle(__inca_native__.createNode('div'))[0];
                return JSON.stringify(e.getBoundingClientRect());
            })()
            ",
        );

        assert_eq!(
            got,
            json!(r#"{"x":0,"y":0,"width":0,"height":0,"top":0,"right":0,"bottom":0,"left":0}"#)
        );
    }

    #[test]
    fn a_write_to_the_rect_moves_its_edges() {
        let (engine, _host, _id) = with_box([10.0, 0.0, 30.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0]);

        let got = read(
            &engine,
            r"
            (() => {
                const r = el.getBoundingClientRect();
                r.x = 5;
                const moved = [r.left, r.right];
                r.width = -20;
                return [moved, r.left, r.right, JSON.parse(JSON.stringify(r)).x];
            })()
            ",
        );

        assert_eq!(got, json!([[5, 35], -15, 5, 5]));
    }
}
