// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bundles each `.vue` file in `ui-fixtures/` once
//! per test binary and mounts the result in a [`Harness`].

#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use gpui::TestAppContext;
use inca_host::harness::Harness;
use inca_host::snapshot::Node;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

static BUNDLES: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Builds every fixture once, reads the bundles into memory and removes the
/// build directory. Panics with Node's stderr when the build fails.
fn bundles() -> &'static HashMap<String, String> {
    BUNDLES.get_or_init(|| {
        let dist = format!("{ROOT}/packages/core/dist");
        assert!(
            Path::new(&dist).exists(),
            "missing build output: {dist} — run `pnpm -F incajs build` first"
        );

        let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("ui-bundles-{}", std::process::id()));
        drop(fs::remove_dir_all(&out));
        let run = Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/build-ui-fixtures.mjs"
            ))
            .arg(&out)
            .output()
            .expect("node is needed to build the UI fixtures");
        assert!(
            run.status.success(),
            "building the UI fixtures failed:\n{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let bundles = fs::read_dir(&out)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "js"))
            .map(|path| {
                let name = path.file_stem().unwrap().to_string_lossy().into_owned();
                (name, fs::read_to_string(&path).unwrap())
            })
            .collect();
        drop(fs::remove_dir_all(&out));
        bundles
    })
}

fn bundle(name: &str) -> &'static str {
    bundles()
        .get(name)
        .unwrap_or_else(|| panic!("no fixture {name}.vue"))
}

/// Mounts the fixture `name` and settles it.
pub fn load(cx: &mut TestAppContext, name: &str) -> Harness {
    Harness::load(cx, &format!("/{name}/bundle.js"), bundle(name))
}

/// The node whose `id` attribute is `id`, if any.
fn find_id<'a>(root: &'a Node, id: &str) -> Option<&'a Node> {
    root.find(&|n| n.attributes.get("id").and_then(|v| v.as_str()) == Some(id))
}

/// The node whose `id` attribute is `id`.
pub fn by_id<'a>(root: &'a Node, id: &str) -> &'a Node {
    find_id(root, id).unwrap_or_else(|| panic!("no node with id {id}"))
}

/// Whether a node with `id` exists.
pub fn has_id(root: &Node, id: &str) -> bool {
    find_id(root, id).is_some()
}

/// Clicks the node with `id`.
pub fn click(h: &mut Harness, id: &str) {
    let node = by_id(&h.snapshot(), id).id;
    h.click(node);
}

/// Asserts `a` and `b` differ by less than 0.01.
pub fn near(a: f32, b: f32) {
    assert!((a - b).abs() < 0.01, "{a} is not {b}");
}

/// The text of every text node under `node`, joined in tree order.
pub fn text(node: &Node) -> String {
    let mut out = node.text.clone().unwrap_or_default();
    for child in &node.children {
        out.push_str(&text(child));
    }
    out.trim().to_owned()
}

/// The text under the node with `id`, from a fresh snapshot.
pub fn text_of(h: &mut Harness, id: &str) -> String {
    text(by_id(&h.snapshot(), id))
}

/// The `id` attributes of `node`'s children that have one, in order.
pub fn child_ids(node: &Node) -> Vec<String> {
    node.children
        .iter()
        .filter_map(|c| c.attributes.get("id")?.as_str().map(str::to_owned))
        .collect()
}
