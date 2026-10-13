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

/// Defines `__inca_test_listen__(node, type, fn, capture, passive)`, which
/// registers `fn` through `__inca_native__.addEventListener`.
pub(crate) const LISTEN_JS: &str = r"
    globalThis.__inca_test_listen__ = (node, type, fn, capture = false, passive = false) => {
        globalThis.__inca_callbacks__ ??= {};
        const id = __inca_native__.addEventListener(node, type, capture, passive);
        globalThis.__inca_callbacks__[id] = fn;
    };
    ";

/// An engine with the bindings installed on `host` and [`LISTEN_JS`] evaluated.
pub(crate) fn engine_for(host: &Rc<RefCell<Host>>) -> Rc<Engine> {
    let engine = Rc::new(Engine::new().unwrap());
    engine.with(|ctx| install(&ctx, host)).unwrap();
    engine.eval::<()>(LISTEN_JS).unwrap();
    engine
}
