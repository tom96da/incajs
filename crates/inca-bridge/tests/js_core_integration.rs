// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exercises `Engine::eval_module` against `packages/core`'s actual
//! compiled output (a real Rolldown/Vite bundle), not hand-written JS strings
//! like `bindings.rs`'s own tests use. A real bundler renames a module's
//! internal top-level bindings (e.g. `createNode` becomes some single-letter
//! name), so its exported functions are only reachable through the module's
//! export table — this test reads them back via `Module::get` inside the
//! same `Engine::with` call that evaluates the module, then invokes them and
//! asserts on the resulting `VirtualTree` state.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use inca_bridge::Host;
use inca_bridge::bindings::install;
use inca_gpui::AttributeValue;
use inca_jsenv::Engine;
use rquickjs::{Function, Module};

const BUNDLE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../packages/core/dist/index.js"
);

#[test]
fn compiled_inca_bundle_drives_the_real_virtual_tree() {
    let source = fs::read_to_string(BUNDLE_PATH).unwrap_or_else(|_| {
        panic!("{BUNDLE_PATH} is missing — run `pnpm --filter incajs build` first")
    });

    let host = Rc::new(RefCell::new(Host::default()));
    let engine = Engine::new().unwrap();
    engine.with(|ctx| install(&ctx, &host)).unwrap();

    let node_id: u32 = engine
        .with(|ctx| -> rquickjs::Result<u32> {
            let (module, promise) = Module::declare(ctx, "inca-core.mjs", source)?.eval()?;
            promise.finish::<()>()?;

            let create_node: Function = module.get("createNode")?;
            let set_attribute: Function = module.get("setAttribute")?;

            let node_id: u32 = create_node.call(("div",))?;
            set_attribute.call::<_, ()>((node_id, "value", "hi"))?;
            Ok(node_id)
        })
        .unwrap();

    let host = host.borrow();
    let node = host.tree.get(node_id).unwrap();
    assert_eq!(node.tag_name(), "div");
    assert_eq!(
        node.attributes().get("value"),
        Some(&AttributeValue::from("hi"))
    );
}
