// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared setup for tests: an engine wired to a [`Host`] through the bindings,
//! with `__inca_test_listen__` for registering listeners.

use std::cell::RefCell;
use std::rc::Rc;

use inca_jsenv::Engine;

use crate::Host;
use crate::bindings::install;

/// Defines `__inca_test_listen__(node, type, fn, capture, passive)`, which
/// registers `fn` through `__inca_native__.addEventListener`.
pub const LISTEN_JS: &str = r"
    globalThis.__inca_test_listen__ = (node, type, fn, capture = false, passive = false) => {
        globalThis.__inca_callbacks__ ??= {};
        const id = __inca_native__.addEventListener(node, type, capture, passive);
        globalThis.__inca_callbacks__[id] = fn;
    };
    ";

/// An engine with the bindings installed on a fresh default [`Host`], returned
/// together with that host.
///
/// # Panics
///
/// Panics when the engine or the bindings fail to initialise.
#[must_use]
pub fn engine_with_bindings() -> (Engine, Rc<RefCell<Host>>) {
    let engine = Engine::new().unwrap();
    let host = Rc::new(RefCell::new(Host::default()));
    engine.with(|ctx| install(&ctx, &host)).unwrap();
    (engine, host)
}

/// An engine with the bindings installed on `host` and [`LISTEN_JS`] evaluated.
///
/// # Panics
///
/// Panics when the engine or the bindings fail to initialise.
pub fn engine_for(host: &Rc<RefCell<Host>>) -> Rc<Engine> {
    let engine = Rc::new(Engine::new().unwrap());
    engine.with(|ctx| install(&ctx, host)).unwrap();
    engine.eval::<()>(LISTEN_JS).unwrap();
    engine
}
