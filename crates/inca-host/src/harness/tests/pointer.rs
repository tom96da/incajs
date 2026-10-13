// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use std::collections::BTreeMap;

/// A 100x50 box holding a text leaf, with a click listener that counts.
const CLICKABLE: &str = r"
        const n = __inca_native__;
        const box = n.createNode('div');
        n.setStyle(box, 'width', 100);
        n.setStyle(box, 'height', 50);
        n.appendChild(n.rootNodeId(), box);
        const label = n.createNode('text');
        n.setAttribute(label, 'value', 'hi');
        n.appendChild(box, label);
        globalThis.clicks = 0;
        globalThis.__inca_callbacks__ = { 0: () => { globalThis.clicks++; } };
        n.addEventListener(box, 'click');
    ";

#[gpui::test]
fn the_root_layout_spans_the_viewport_width_after_a_render(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, CLICKABLE);

    let width = eval_f64(
        &h,
        "__inca_native__.layoutOf(__inca_native__.rootNodeId())[2]",
    );
    let viewport =
        h.cx.update(|window, _| f64::from(f32::from(window.viewport_size().width)));

    assert_eq!(width, viewport);
}

#[gpui::test]
fn load_mounts_the_bundle_and_reports_bounds(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, CLICKABLE);

    let snap = h.snapshot();
    let boxed = snap
        .find(&|n| n.children.iter().any(|c| c.text.is_some()))
        .unwrap();
    let b = boxed.bounds.unwrap();
    assert_eq!((b.width, b.height), (100.0, 50.0));
    assert_eq!(boxed.children[0].text.as_deref(), Some("hi"));
    assert_eq!(boxed.children[0].bounds, None);
}

#[gpui::test]
#[should_panic(expected = "the bundle failed to start")]
fn load_panics_when_the_bundle_throws(cx: &mut TestAppContext) {
    Harness::load(cx, TEST_ENTRY_PATH, "throw new Error('boom');");
}

#[gpui::test]
fn click_reaches_a_listener_on_the_node(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, CLICKABLE);
    let boxed = id_where(&mut h, |n| {
        n.tag == "div" && n.bounds.is_some_and(|b| b.width == 100.0)
    });

    h.click(boxed);

    assert_eq!(eval_f64(&h, "globalThis.clicks"), 1.0);
}

#[gpui::test]
fn click_on_a_text_leaf_lands_on_its_container(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, CLICKABLE);
    let leaf = id_where(&mut h, |n| n.text.is_some());

    h.click(leaf);

    assert_eq!(eval_f64(&h, "globalThis.clicks"), 1.0);
}

#[gpui::test]
#[should_panic(expected = "no node 9999 in the tree")]
fn click_panics_on_an_unknown_node(cx: &mut TestAppContext) {
    Harness::load(cx, TEST_ENTRY_PATH, CLICKABLE).click(9999);
}

/// A 100x50 box below a spacer, counting its `mouseenter` and `mouseleave`
/// events. The pointer starts over the spacer.
const HOVERABLE: &str = r"
        const n = __inca_native__;
        const frame = n.createNode('div');
        n.setStyle(frame, 'width', 200);
        n.setStyle(frame, 'height', 200);
        n.appendChild(n.rootNodeId(), frame);
        const spacer = n.createNode('div');
        n.setStyle(spacer, 'width', 60);
        n.setStyle(spacer, 'height', 50);
        n.appendChild(frame, spacer);
        const box = n.createNode('div');
        n.setStyle(box, 'width', 100);
        n.setStyle(box, 'height', 50);
        n.appendChild(frame, box);
        globalThis.enters = 0;
        globalThis.leaves = 0;
        globalThis.__inca_callbacks__ = {
            0: () => { globalThis.enters++; },
            1: () => { globalThis.leaves++; },
        };
        n.addEventListener(box, 'mouseenter');
        n.addEventListener(box, 'mouseleave');
    ";

fn hover_box(h: &mut Harness) -> NodeId {
    id_where(h, |n| n.bounds.is_some_and(|b| b.width == 100.0))
}

#[gpui::test]
fn hover_fires_mouseenter_once(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, HOVERABLE);
    let boxed = hover_box(&mut h);
    assert_eq!(eval_f64(&h, "globalThis.enters"), 0.0);

    h.hover(boxed);

    assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
    assert_eq!(eval_f64(&h, "globalThis.leaves"), 0.0);
}

#[gpui::test]
fn hovering_the_same_node_again_does_not_repeat_mouseenter(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, HOVERABLE);
    let boxed = hover_box(&mut h);

    h.hover(boxed);
    h.hover(boxed);

    assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
}

#[gpui::test]
fn unhover_fires_mouseleave(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, HOVERABLE);
    let boxed = hover_box(&mut h);
    h.hover(boxed);

    h.unhover();

    assert_eq!(eval_f64(&h, "globalThis.leaves"), 1.0);
}

#[gpui::test]
fn hover_again_after_leaving_fires_mouseenter_again(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, HOVERABLE);
    let boxed = hover_box(&mut h);

    h.hover(boxed);
    h.unhover();
    h.hover(boxed);

    assert_eq!(eval_f64(&h, "globalThis.enters"), 2.0);
    assert_eq!(eval_f64(&h, "globalThis.leaves"), 1.0);
}

#[gpui::test]
fn unhover_twice_fires_mouseleave_once(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, TEST_ENTRY_PATH, HOVERABLE);
    let boxed = hover_box(&mut h);
    h.hover(boxed);

    h.unhover();
    h.unhover();

    assert_eq!(eval_f64(&h, "globalThis.leaves"), 1.0);
}

#[gpui::test]
fn unhover_before_any_hover_fires_nothing(cx: &mut TestAppContext) {
    let mut h = Harness::load(
        cx,
        TEST_ENTRY_PATH,
        r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.setStyle(box, 'margin_left', 200);
            n.appendChild(n.rootNodeId(), box);
            globalThis.leaves = 0;
            globalThis.__inca_callbacks__ = { 0: () => { globalThis.leaves++; } };
            n.addEventListener(box, 'mouseleave');
            ",
    );

    h.unhover();

    assert_eq!(eval_f64(&h, "globalThis.leaves"), 0.0);
}

#[gpui::test]
fn hovering_a_child_enters_its_ancestors(cx: &mut TestAppContext) {
    let mut h = Harness::load(
        cx,
        TEST_ENTRY_PATH,
        r"
            const n = __inca_native__;
            const frame = n.createNode('div');
            n.setStyle(frame, 'width', 200);
            n.setStyle(frame, 'height', 200);
            n.appendChild(n.rootNodeId(), frame);
            const spacer = n.createNode('div');
            n.setStyle(spacer, 'width', 100);
            n.setStyle(spacer, 'height', 50);
            n.appendChild(frame, spacer);
            const outer = n.createNode('div');
            n.setStyle(outer, 'width', 100);
            n.setStyle(outer, 'height', 100);
            n.appendChild(frame, outer);
            const inner = n.createNode('div');
            n.setStyle(inner, 'width', 20);
            n.setStyle(inner, 'height', 20);
            n.appendChild(outer, inner);
            globalThis.enters = 0;
            globalThis.__inca_callbacks__ = { 0: () => { globalThis.enters++; } };
            n.addEventListener(outer, 'mouseenter');
            ",
    );
    assert_eq!(eval_f64(&h, "globalThis.enters"), 0.0);
    let inner = id_where(&mut h, |n| n.bounds.is_some_and(|b| b.width == 20.0));

    h.hover(inner);

    assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
}

#[gpui::test]
fn hover_on_a_text_leaf_lands_on_its_container(cx: &mut TestAppContext) {
    let mut h = Harness::load(
        cx,
        TEST_ENTRY_PATH,
        &format!(
            r"
            {HOVERABLE}
            const label = n.createNode('text');
            n.setAttribute(label, 'value', 'hi');
            n.appendChild(box, label);
            "
        ),
    );
    let leaf = id_where(&mut h, |n| n.text.is_some());

    h.hover(leaf);

    assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
}

#[gpui::test]
#[should_panic(expected = "no node 9999 in the tree")]
fn hover_panics_on_an_unknown_node(cx: &mut TestAppContext) {
    Harness::load(cx, TEST_ENTRY_PATH, HOVERABLE).hover(9999);
}

#[test]
fn nearest_bounds_inherits_from_the_closest_ancestor() {
    let b = |x| Bounds {
        x,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };
    let node = |id, bounds, children| Node {
        id,
        tag: "div".into(),
        text: None,
        attributes: BTreeMap::new(),
        bounds,
        children,
    };
    let tree = node(
        1,
        Some(b(1.0)),
        vec![node(2, Some(b(2.0)), vec![node(3, None, vec![])])],
    );

    assert_eq!(nearest_bounds(&tree, 3, None), Lookup::At(b(2.0)));
    assert_eq!(nearest_bounds(&tree, 1, None), Lookup::At(b(1.0)));
    assert_eq!(nearest_bounds(&tree, 9, None), Lookup::Absent);
    let bare = node(1, None, vec![node(2, None, vec![])]);
    assert_eq!(nearest_bounds(&bare, 2, None), Lookup::Unbounded);
}

#[gpui::test]
fn a_zero_size_node_resolves_to_its_own_point(cx: &mut TestAppContext) {
    let mut h = Harness::load(
        cx,
        TEST_ENTRY_PATH,
        r"
            const n = __inca_native__;
            const z = n.createNode('div');
            n.setStyle(z, 'width', 0);
            n.setStyle(z, 'height', 0);
            n.appendChild(n.rootNodeId(), z);
            ",
    );
    let snap = h.snapshot();
    let z = snap.children[0].clone();
    let b = z.bounds.unwrap();
    assert_eq!((b.width, b.height), (0.0, 0.0));

    assert_eq!(h.center_of(z.id), point(px(b.x), px(b.y)));
}
