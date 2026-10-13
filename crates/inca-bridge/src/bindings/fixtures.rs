// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use inca_jsenv::Engine;

pub(super) fn engine_with_bindings() -> (Engine, Rc<RefCell<Host>>) {
    let engine = Engine::new().unwrap();
    let host = Rc::new(RefCell::new(Host::default()));
    engine.with(|ctx| install(&ctx, &host)).unwrap();
    (engine, host)
}
