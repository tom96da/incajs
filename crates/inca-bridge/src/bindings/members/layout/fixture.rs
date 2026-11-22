// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The engine and the recorded box that the tests of the layout members share.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use inca_document::NodeBox;
use inca_document::geometry::Rect;
use inca_jsenv::Engine;

use crate::bindings::host::Host;
use crate::test_support::{TestView, engine_with_view};

/// An engine with the global `el`, a `div` under the root, whose last box is `b`
/// (`x`, `y`, `width`, `height`, `border`, `scroll x`, `scroll y`, `gutter w`, `gutter h`).
pub(super) fn with_box(b: [f32; 9]) -> (Engine, Rc<Host>, u32) {
    let (engine, host, _view, id) = with_box_view(b);
    (engine, host, id)
}

/// [`with_box`] with the view that holds the box.
pub(super) fn with_box_view(b: [f32; 9]) -> (Engine, Rc<Host>, Rc<RefCell<TestView>>, u32) {
    let (engine, host, view) = engine_with_view();
    let id: u32 = engine
        .eval(
            r"
            const n = __inca_native__;
            globalThis.id = n.createNode('div');
            n.appendChild(n.rootNodeId(), id);
            globalThis.el = n.nodeHandle(id)[0];
            n.setStyle(id, 'overflow', 'scroll');
            id;
            ",
        )
        .unwrap();
    record(&view, id, b);
    (engine, host, view, id)
}

pub(super) fn record(view: &Rc<RefCell<TestView>>, id: u32, b: [f32; 9]) {
    let node_box = NodeBox {
        rect: Rect {
            x: b[0],
            y: b[1],
            width: b[2],
            height: b[3],
        },
        border: b[4],
        scroll: (b[5], b[6]),
        gutter: (b[7], b[8]),
    };
    view.borrow_mut().boxes.insert(id, node_box);
}

pub(super) fn read(engine: &Engine, expr: &str) -> serde_json::Value {
    let text: String = engine.eval(&format!("JSON.stringify({expr})")).unwrap();
    serde_json::from_str(&text).unwrap()
}
