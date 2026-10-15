// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[gpui::test]
fn stop_propagation_skips_ancestors_but_not_the_scroll(cx: &mut TestAppContext) {
    let host = one_container();
    let js = [on_wheel(0, CONTAINER), on_wheel(1, CHILD_STOP)].concat();
    let mut scene = Scene::new(cx, host, &js);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child"]"#);
    assert_eq!(scene.offset(0).y, px(-50.));

    scene.wheel(-20.);
    assert_eq!(
        scene.ran(),
        r#"["child","child"]"#,
        "the stop ends with its event"
    );
    assert_eq!(scene.offset(0).y, px(-70.));
}

#[gpui::test]
fn prevent_default_from_a_child_cancels_the_scroll(cx: &mut TestAppContext) {
    let host = one_container();
    let js = on_wheel(1, CHILD_PREVENT);
    let mut scene = Scene::new(cx, host, &js);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child"]"#);
    assert_eq!(scene.offset(0).y, px(0.));
}

#[gpui::test]
fn a_passive_listener_cannot_cancel_the_scroll(cx: &mut TestAppContext) {
    let host = one_container();
    let js = listen(1, "wheel", CHILD_PREVENT, false, true);
    let mut scene = Scene::new(cx, host, &js);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child"]"#);
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn prevent_default_from_a_capture_listener_of_the_container_cancels_the_scroll(
    cx: &mut TestAppContext,
) {
    let host = one_container();
    let js = [
        listen(0, "wheel", CONTAINER_PREVENT, true, false),
        on_wheel(1, SECOND),
    ]
    .concat();
    let mut scene = Scene::new(cx, host, &js);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["container","second"]"#);
    assert_eq!(scene.offset(0).y, px(0.));
}

#[gpui::test]
fn a_script_offset_past_the_end_still_lets_the_ancestor_scroll(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, nested_containers(), "");
    // No draw between the script offset and the wheel, so the offset is unclamped.
    scene.js("__inca_native__.scrollNode(1, null, 999)");
    scene.cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.), px(10.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-50.))),
        ..Default::default()
    });
    scene.cx.run_until_parked();
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn prevent_default_from_the_inner_container_cancels_both_scrolls(cx: &mut TestAppContext) {
    let host = nested_containers();
    let js = on_wheel(1, CHILD_PREVENT);
    let mut scene = Scene::new(cx, host, &js);
    scene.wheel(-50.);
    scene.wheel(-250.);
    assert_eq!(scene.offset(1).y, px(0.));
    assert_eq!(scene.offset(0).y, px(0.));
}

#[gpui::test]
fn stop_propagation_from_the_container_itself_keeps_its_other_listeners(cx: &mut TestAppContext) {
    let host = one_container();
    let js = [on_wheel(0, CONTAINER_STOP), on_wheel(0, SECOND)].concat();
    let mut scene = Scene::new(cx, host, &js);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["container","second"]"#);
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn stop_immediate_propagation_from_a_child_still_scrolls(cx: &mut TestAppContext) {
    let host = one_container();
    let js = [
        on_wheel(0, CONTAINER),
        on_wheel(1, IMMEDIATE),
        on_wheel(1, SECOND),
    ]
    .concat();
    let mut scene = Scene::new(cx, host, &js);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["immediate"]"#);
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn a_stopped_wheel_does_not_silence_the_next_one(cx: &mut TestAppContext) {
    let host = one_container();
    let js = [on_wheel(0, CONTAINER), on_wheel(1, CHILD_STOP_FLAG)].concat();
    let mut scene = Scene::new(cx, host, &js);
    scene.js("globalThis.stop = true;");
    scene.wheel(-10.);
    scene.js("globalThis.stop = false;");
    scene.wheel(-10.);
    assert_eq!(scene.ran(), r#"["child","child","container"]"#);
    assert_eq!(scene.offset(0).y, px(-20.));
}

#[gpui::test]
fn prevent_default_from_an_ancestor_cancels_the_scroll(cx: &mut TestAppContext) {
    let mut host = Host::default();
    let root = add_div(&mut host, None, 100.0, false);
    let container = add_div(&mut host, Some(root), 100.0, true);
    add_div(&mut host, Some(container), 300.0, false);
    let js = on_wheel(root, CHILD_PREVENT);
    let mut scene = Scene::new(cx, host, &js);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child"]"#);
    assert_eq!(scene.offset(container).y, px(0.));
}

#[gpui::test]
fn a_wheel_after_a_prevented_one_scrolls_again(cx: &mut TestAppContext) {
    let host = one_container();
    let js = on_wheel(0, PREVENT_FLAG);
    let mut scene = Scene::new(cx, host, &js);
    scene.js("globalThis.prevent = true;");
    scene.wheel(-50.);
    assert_eq!(scene.offset(0).y, px(0.));

    scene.js("globalThis.prevent = false;");
    scene.wheel(-50.);
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn a_throw_after_stop_propagation_still_stops_ancestors_and_scrolls(cx: &mut TestAppContext) {
    let host = one_container();
    let js = [on_wheel(0, CONTAINER), on_wheel(1, CHILD_THROW)].concat();
    let mut scene = Scene::new(cx, host, &js);
    scene.js("globalThis.stop = true;");
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child"]"#);
    assert_eq!(scene.offset(0).y, px(-50.));
    assert_eq!(scene.reported.borrow().len(), 1);
    assert_eq!(scene.reported.borrow()[0].message(), "Error: boom");

    scene.js("globalThis.stop = false;");
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["child","child","container"]"#);
    assert_eq!(scene.offset(0).y, px(-100.));
    assert_eq!(scene.reported.borrow().len(), 1);
}

/// Records only.
const CONTAINER: &str = "() => { ran.push('container'); }";
/// Stops propagation.
const CHILD_STOP: &str = "(e) => { ran.push('child'); e.stopPropagation(); }";
/// Prevents default.
const CHILD_PREVENT: &str = "(e) => { ran.push('child'); e.preventDefault(); }";
/// Prevents default.
const CONTAINER_PREVENT: &str = "(e) => { ran.push('container'); e.preventDefault(); }";
/// A second listener on the same node.
const SECOND: &str = "() => { ran.push('second'); }";
/// Stops immediately.
const IMMEDIATE: &str = "(e) => { ran.push('immediate'); e.stopImmediatePropagation(); }";
/// Stops propagation.
const CONTAINER_STOP: &str = "(e) => { ran.push('container'); e.stopPropagation(); }";
/// Prevents default while `globalThis.prevent` is set.
const PREVENT_FLAG: &str =
    "(e) => { ran.push('prevent'); if (globalThis.prevent) e.preventDefault(); }";
/// Stops propagation while `globalThis.stop` is set.
const CHILD_STOP_FLAG: &str =
    "(e) => { ran.push('child'); if (globalThis.stop) e.stopPropagation(); }";
/// Stops propagation, then throws, while `globalThis.stop` is set.
const CHILD_THROW: &str = r"(e) => {
    ran.push('child');
    if (globalThis.stop) { e.stopPropagation(); throw new Error('boom'); }
}";
