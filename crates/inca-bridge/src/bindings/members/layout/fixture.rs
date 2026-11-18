// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The engine and the recorded box that the tests of the layout members share.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Bounds, point, px, size};
use inca_gpui::NodeLayout;
use inca_jsenv::Engine;

use crate::bindings::host::Host;
use crate::test_support::engine_with_bindings;

/// An engine with the global `el`, a `div` under the root, whose last box is `b`
/// (`x`, `y`, `width`, `height`, `border`, `scroll x`, `scroll y`, `gutter w`, `gutter h`).
pub(super) fn with_box(b: [f32; 9]) -> (Engine, Rc<RefCell<Host>>, u32) {
    let (engine, host) = engine_with_bindings();
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
    record(&host, id, b);
    (engine, host, id)
}

pub(super) fn record(host: &Rc<RefCell<Host>>, id: u32, b: [f32; 9]) {
    let layout = NodeLayout {
        bounds: Bounds::new(point(px(b[0]), px(b[1])), size(px(b[2]), px(b[3]))),
        border: px(b[4]),
        scroll: point(px(b[5]), px(b[6])),
        gutter: size(px(b[7]), px(b[8])),
    };
    host.borrow().layout.borrow_mut().insert(id, layout);
}

pub(super) fn read(engine: &Engine, expr: &str) -> serde_json::Value {
    let text: String = engine.eval(&format!("JSON.stringify({expr})")).unwrap();
    serde_json::from_str(&text).unwrap()
}
