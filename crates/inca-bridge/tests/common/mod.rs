// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared setup for the integration tests: an engine wired to a host through
//! the bindings, with `__inca_test_listen__` for registering listeners.

use std::cell::RefCell;
use std::rc::Rc;

use inca_bridge::Host;
use inca_jsenv::Engine;

/// Defines `__inca_test_listen__(node, type, fn, capture, passive)`.
pub const LISTEN_JS: &str = r"
    globalThis.__inca_test_listen__ = (node, type, fn, capture = false, passive = false) => {
        globalThis.__inca_callbacks__ ??= {};
        const id = __inca_native__.addEventListener(node, type, capture, passive);
        globalThis.__inca_callbacks__[id] = fn;
    };
    ";

/// An engine with `__inca_native__` bound to `host` and `LISTEN_JS` loaded.
pub fn engine_for(host: &Rc<RefCell<Host>>) -> Rc<Engine> {
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .with(|ctx| inca_bridge::bindings::install(&ctx, host))
        .unwrap();
    engine.eval::<()>(LISTEN_JS).unwrap();
    engine
}
