// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exercises `Engine::eval_module` against `packages/core`'s actual
//! compiled output, a real Rolldown/Vite bundle. The tests of the `bindings`
//! module use hand-written JS strings. A real bundler renames a module's internal
//! top-level bindings (e.g. `createNode` becomes some single-letter name),
//! so its exported functions are reachable only through the module's export
//! table. This test reads them back via `Module::get` inside the same
//! `Engine::with` call that evaluates the module, then invokes them and
//! asserts on the resulting `VirtualTree` state.
//!
//! It loads the `rendererCore` entry, the bundle entry that exports the tree
//! calls by name.

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::Path;
use std::rc::Rc;

use inca_bridge::Host;
use inca_bridge::install;
use inca_jsenv::Engine;
use inca_node::AttributeValue;
use rquickjs::{Function, Module};
use rstest::rstest;

const BUNDLE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../packages/core/dist/rendererCore.js"
);

#[test]
fn compiled_inca_bundle_drives_the_real_virtual_tree() {
    let source = fs::read_to_string(BUNDLE_PATH).unwrap_or_else(|_| {
        panic!("{BUNDLE_PATH} is missing. run `pnpm --filter incajs build` first")
    });
    let dist_dir = Path::new(BUNDLE_PATH).parent().unwrap();

    let host = Rc::new(Host::default());
    let engine = Engine::builder().module_root(dist_dir).build().unwrap();
    engine.with(|ctx| install(&ctx, &host)).unwrap();

    let node_id: u32 = engine
        .with(|ctx| -> rquickjs::Result<u32> {
            let (module, promise) = Module::declare(ctx, BUNDLE_PATH, source)?.eval()?;
            promise.finish::<()>()?;

            let create_node: Function = module.get("createNode")?;
            let set_attribute: Function = module.get("setAttribute")?;

            let node_id: u32 = create_node.call(("div",))?;
            set_attribute.call::<_, ()>((node_id, "value", "hi"))?;
            Ok(node_id)
        })
        .unwrap();

    let host = host.document.borrow();
    let node = host.tree.get(node_id).unwrap();
    assert_eq!(node.local_name(), "div");
    assert_eq!(
        node.attributes().get("value"),
        Some(&AttributeValue::from("hi"))
    );
}

/// Loads the bundle into a real host and gives the script its exports as `core`.
fn engine_with_core() -> (Engine, Rc<Host>) {
    let source = fs::read_to_string(BUNDLE_PATH).unwrap_or_else(|_| {
        panic!("{BUNDLE_PATH} is missing. run `pnpm --filter incajs build` first")
    });
    let dist_dir = Path::new(BUNDLE_PATH).parent().unwrap();
    let host = Rc::new(Host::default());
    let engine = Engine::builder().module_root(dist_dir).build().unwrap();
    engine.with(|ctx| install(&ctx, &host)).unwrap();
    engine
        .with(|ctx| -> rquickjs::Result<()> {
            let (module, promise) = Module::declare(ctx.clone(), BUNDLE_PATH, source)?.eval()?;
            promise.finish::<()>()?;
            ctx.globals().set("core", module.namespace()?)
        })
        .unwrap();
    (engine, host)
}

#[test]
fn append_gives_the_host_the_text_node_of_a_string() {
    let (engine, host) = engine_with_core();

    let outcome: String = engine
        .eval(
            r#"
            const p = core.createElementNode("div");
            const x = core.createElementNode("span");
            p.append("a", x);
            JSON.stringify([p.firstChild.data, p.lastChild === x, p.__node_id, x.__node_id]);
            "#,
        )
        .unwrap();

    let (data, last_is_x, p, x): (String, bool, u32, u32) = serde_json::from_str(&outcome).unwrap();
    let host = host.document.borrow();
    let children = host.tree.get(p).unwrap().children();
    assert_eq!((data.as_str(), last_is_x), ("a", true));
    assert_eq!(host.tree.get(children[0]).unwrap().local_name(), "#text");
    assert_eq!(children[1], x);
}

#[test]
fn replace_children_leaves_no_child() {
    let (engine, host) = engine_with_core();

    let outcome: String = engine
        .eval(
            r#"
            const p = core.createElementNode("div");
            const x = core.createElementNode("span");
            p.append("a", x);
            p.replaceChildren();
            JSON.stringify([p.__node_id, x.parentNode === null]);
            "#,
        )
        .unwrap();

    let (p, detached): (u32, bool) = serde_json::from_str(&outcome).unwrap();
    assert_eq!(
        host.document.borrow().tree.get(p).unwrap().children().len(),
        0
    );
    assert!(detached);
}

/// Runs `statement` that writes a string with a lone surrogate, and reads the error name, the
/// data of a text node made from `"a"` and the children of a `div`.
fn lone_surrogate_outcome(statement: &str) -> String {
    let (engine, _host) = engine_with_core();
    engine
        .eval(&format!(
            r#"
            const t = core.createTextNode("a");
            const p = core.createElementNode("div");
            let name = "none";
            try {{
                {statement};
            }} catch (e) {{
                name = e.name;
            }}
            JSON.stringify([name, t.data, p.childNodes.length]);
            "#
        ))
        .unwrap()
}

#[rstest]
#[case::create_text_node(r#"core.createTextNode("\ud800")"#)]
#[case::data(r#"t.data = "\ud800""#)]
#[case::node_value(r#"t.nodeValue = "\ud800""#)]
#[case::text_content(r#"t.textContent = "\ud800""#)]
#[case::append_data(r#"t.appendData("\ud800")"#)]
#[case::insert_data(r#"t.insertData(0, "\ud800")"#)]
#[case::replace_data(r#"t.replaceData(0, 1, "\ud800")"#)]
#[case::set_attribute(r#"p.setAttribute("title", "\ud800")"#)]
#[case::append(r#"p.append("\ud800")"#)]
fn a_member_throws_a_type_error_for_a_lone_surrogate(#[case] statement: &str) {
    let outcome = lone_surrogate_outcome(statement);

    assert_eq!(outcome, r#"["TypeError","a",0]"#);
}
