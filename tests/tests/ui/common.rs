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

use gpui::{Modifiers, MouseButton, Pixels, Point, TestAppContext, point, px};
use inca_host::harness::Harness;
use inca_host::snapshot::Node;
use inca_node::NodeId;
use serde_json::Value;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

static BUNDLES: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Builds every fixture once, reads the bundles into memory and removes the
/// build directory. Panics with Node's stderr when the build fails.
fn bundles() -> &'static HashMap<String, String> {
    BUNDLES.get_or_init(|| {
        let dist = format!("{ROOT}/packages/core/dist");
        assert!(
            Path::new(&dist).exists(),
            "missing build output: {dist}. run `pnpm -F incajs build` first"
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

/// The id of the node whose `id` attribute is `id`, from a fresh snapshot.
pub fn node_id(h: &mut Harness, id: &str) -> NodeId {
    by_id(&h.snapshot(), id).id
}

/// Clicks the node with `id`.
pub fn click(h: &mut Harness, id: &str) {
    let node = node_id(h, id);
    h.click(node);
}

/// Clicks the node with `id` with `button`, `modifiers` and `count`.
pub fn click_with(
    h: &mut Harness,
    id: &str,
    button: MouseButton,
    modifiers: Modifiers,
    count: usize,
) {
    let node = node_id(h, id);
    h.click_with(node, button, modifiers, count);
}

/// Moves the pointer onto the node with `id`.
pub fn hover(h: &mut Harness, id: &str) {
    let node = node_id(h, id);
    h.hover(node);
}

/// Mounts the fixture `name`, clicks its `#probe` and parses the JSON that
/// the probe writes into `#result`.
pub fn probe(cx: &mut TestAppContext, name: &str) -> Value {
    let mut h = load(cx, name);
    click(&mut h, "probe");
    serde_json::from_str(&text_of(&mut h, "result")).unwrap()
}

/// The left edge of the node with `id`.
pub fn x(h: &mut Harness, id: &str) -> f32 {
    by_id(&h.snapshot(), id).bounds.unwrap().x
}

/// The top edge of the node with `id`.
pub fn y(h: &mut Harness, id: &str) -> f32 {
    by_id(&h.snapshot(), id).bounds.unwrap().y
}

/// The width of the node with `id`.
pub fn width(h: &mut Harness, id: &str) -> f32 {
    by_id(&h.snapshot(), id).bounds.unwrap().width
}

/// The height of the node with `id`.
pub fn height(h: &mut Harness, id: &str) -> f32 {
    by_id(&h.snapshot(), id).bounds.unwrap().height
}

/// The point 2 px right of and below `p`.
pub fn nudge(p: Point<Pixels>) -> Point<Pixels> {
    p + point(px(2.0), px(2.0))
}

/// The center of the node with `id`.
pub fn center(h: &mut Harness, id: &str) -> Point<Pixels> {
    let b = by_id(&h.snapshot(), id).bounds.unwrap();
    point(px(b.x + b.width / 2.0), px(b.y + b.height / 2.0))
}

/// The point `from_right` left of the right edge and `from_top` below the
/// top edge of the node with `id`.
pub fn inset(h: &mut Harness, id: &str, from_right: f32, from_top: f32) -> Point<Pixels> {
    let b = by_id(&h.snapshot(), id).bounds.unwrap();
    point(px(b.x + b.width - from_right), px(b.y + from_top))
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

/// The text of `now` after the prefix `before`.
///
/// # Panics
///
/// Panics when `before` is not a prefix of `now`.
pub fn appended<'a>(before: &str, now: &'a str) -> &'a str {
    now.strip_prefix(before)
        .unwrap_or_else(|| panic!("{before:?} is not a prefix of {now:?}"))
        .trim()
}

/// Settles `frames` times. A request that a listener or a frame callback
/// makes lands one frame later, so a chain of `n` requests needs `n` frames.
pub fn settled(h: &mut Harness, frames: usize) {
    for _ in 0..frames {
        h.settle();
    }
}

/// The top of the node with `id` minus the top of `container`, from one snapshot.
pub fn top_in(h: &mut Harness, id: &str, container: &str) -> f32 {
    let root = h.snapshot();
    by_id(&root, id).bounds.unwrap().y - by_id(&root, container).bounds.unwrap().y
}

/// The left of the node with `id` minus the left of `container`, from one snapshot.
pub fn left_in(h: &mut Harness, id: &str, container: &str) -> f32 {
    let root = h.snapshot();
    by_id(&root, id).bounds.unwrap().x - by_id(&root, container).bounds.unwrap().x
}

/// The distinct whitespace-separated tokens of `log`, sorted.
pub fn sorted_unique(log: &str) -> Vec<String> {
    let mut tokens: Vec<String> = log.split_whitespace().map(str::to_owned).collect();
    tokens.sort();
    tokens.dedup();
    tokens
}
