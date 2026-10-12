// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

mod focus;
mod listeners;
mod scroll;
mod style;
mod tree;

use super::*;
use gpui::Bounds;
use inca_gpui::{AttributeValue, NodeId, NodeLayout};
use inca_jsenv::Engine;

fn engine_with_bindings() -> (Engine, Rc<RefCell<Host>>) {
    let engine = Engine::new().unwrap();
    let host = Rc::new(RefCell::new(Host::default()));
    engine.with(|ctx| install(&ctx, &host)).unwrap();
    (engine, host)
}
