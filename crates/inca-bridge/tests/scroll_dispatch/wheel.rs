// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use inca_test_macros::gpui_cases;

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

/// Node 0 holds node 1, which scrolls 200px over node 2, which scrolls 200px over node 3. Node
/// 4 fills the rest of node 1.
fn scrollers_below_a_plain_root() -> Host {
    let mut host = Host::default();
    let root = add_div(&mut host, None, 100.0, false);
    let outer = add_div(&mut host, Some(root), 100.0, true);
    let inner = add_div(&mut host, Some(outer), 100.0, true);
    add_div(&mut host, Some(inner), 300.0, false);
    add_div(&mut host, Some(outer), 200.0, false);
    host
}

#[gpui_cases]
#[gpui::test]
#[case::outer_container(1, 20, -20.0, 0.0)]
#[case::inner_container(2, 20, 0.0, -20.0)]
#[case::inner_container_at_the_wheel_step_result(2, 30, 0.0, -30.0)]
fn a_prevented_listeners_scroll_write_to_a_container_stays(
    cx: &mut TestAppContext,
    written: NodeId,
    top: u32,
    outer: f32,
    inner: f32,
) {
    let js = on_wheel(
        0,
        &format!(
            r"
            (e) => {{
                e.preventDefault();
                __inca_native__.scrollNode({written}, null, {top});
            }}
            "
        ),
    );
    let mut scene = Scene::new(cx, scrollers_below_a_plain_root(), &js);

    scene.wheel(-30.);

    assert_eq!(scene.offset(1).y, px(outer));
    assert_eq!(scene.offset(2).y, px(inner));
}

#[gpui::test]
fn the_wheel_step_adds_to_an_unprevented_listeners_write_to_the_inner_container(
    cx: &mut TestAppContext,
) {
    let js = on_wheel(
        0,
        r"
        () => {
            __inca_native__.scrollNode(2, null, 20);
        }
        ",
    );
    let mut scene = Scene::new(cx, scrollers_below_a_plain_root(), &js);

    scene.wheel(-30.);

    assert_eq!(scene.offset(2).y, px(-50.));
}

#[gpui::test]
fn a_scroll_write_does_not_outlast_its_wheel_event(cx: &mut TestAppContext) {
    let js = on_wheel(
        0,
        r"
        (e) => {
            if (globalThis.write) {
                __inca_native__.scrollNode(2, null, 20);
            } else {
                e.preventDefault();
            }
        }
        ",
    );
    let mut scene = Scene::new(cx, scrollers_below_a_plain_root(), &js);
    scene.js("globalThis.write = true;");
    scene.wheel(-30.);
    assert_eq!(scene.offset(2).y, px(-50.));

    scene.js("globalThis.write = false;");
    scene.wheel(-30.);

    assert_eq!(scene.offset(2).y, px(-50.));
}

/// Node 0 scrolls 100px over node 1 and node 2, which are 300px and 100px tall.
fn scrolling_root() -> Host {
    let mut host = Host::default();
    let root = add_div(&mut host, None, 100.0, true);
    add_div(&mut host, Some(root), 300.0, false);
    add_div(&mut host, Some(root), 100.0, false);
    host
}

#[gpui_cases]
#[gpui::test]
#[case::a_script_offset("__inca_native__.scrollNode(0, null, 20);", -20.0)]
#[case::an_offset_equal_to_the_wheel_step("__inca_native__.scrollNode(0, null, 30);", -30.0)]
#[case::a_reveal("__inca_native__.scrollIntoViewNode(2, 'start', 'nearest');", -300.0)]
fn a_prevented_listeners_scroll_write_to_the_root_container_stays(
    cx: &mut TestAppContext,
    write: &str,
    expected: f32,
) {
    let js = on_wheel(
        0,
        &format!(
            r"
            (e) => {{
                e.preventDefault();
                {write}
            }}
            "
        ),
    );
    let mut scene = Scene::new(cx, scrolling_root(), &js);

    scene.wheel(-30.);

    assert_eq!(scene.offset(0).y, px(expected));
}

#[gpui::test]
fn the_wheel_step_adds_to_an_unprevented_listeners_write_to_the_root_container(
    cx: &mut TestAppContext,
) {
    let js = on_wheel(
        0,
        r"
        () => {
            __inca_native__.scrollNode(0, null, 20);
        }
        ",
    );
    let mut scene = Scene::new(cx, scrolling_root(), &js);

    scene.wheel(-30.);

    assert_eq!(scene.offset(0).y, px(-50.));
}

/// Node 0 scrolls 100px over node 1, which scrolls 100px over node 2, 300px tall, and over
/// node 3, 200px tall.
fn scrolling_root_over_a_scroller() -> Host {
    let mut host = Host::default();
    let root = add_div(&mut host, None, 100.0, true);
    let inner = add_div(&mut host, Some(root), 100.0, true);
    add_div(&mut host, Some(inner), 300.0, false);
    add_div(&mut host, Some(root), 200.0, false);
    host
}

#[gpui::test]
fn an_unprevented_root_write_stays_when_an_inner_container_takes_the_step(cx: &mut TestAppContext) {
    let js = on_wheel(
        0,
        r"
        () => {
            __inca_native__.scrollNode(0, null, 20);
        }
        ",
    );
    let mut scene = Scene::new(cx, scrolling_root_over_a_scroller(), &js);

    scene.wheel(-30.);

    assert_eq!(scene.offset(0).y, px(-20.));
    assert_eq!(scene.offset(1).y, px(-30.));
}

#[gpui::test]
fn a_script_write_to_an_inert_root_stays(cx: &mut TestAppContext) {
    let mut host = scrolling_root();
    host.tree.set_attribute(0, "inert", true).unwrap();
    let js = on_wheel(
        0,
        r"
        () => {
            __inca_native__.scrollNode(0, null, 20);
        }
        ",
    );
    let mut scene = Scene::new(cx, host, &js);

    scene.wheel(-30.);

    assert_eq!(scene.offset(0).y, px(-20.));
}

#[gpui_cases]
#[gpui::test]
#[case::prevented("e.preventDefault();", 0.0)]
#[case::unprevented("", -30.0)]
fn a_reveal_of_a_shown_node_is_not_a_scroll_write(
    cx: &mut TestAppContext,
    prevent: &str,
    expected: f32,
) {
    let js = on_wheel(
        0,
        &format!(
            r"
            (e) => {{
                {prevent}
                __inca_native__.scrollIntoViewNode(3, 'nearest', 'nearest');
            }}
            "
        ),
    );
    let mut scene = Scene::new(cx, scrollers_below_a_plain_root(), &js);

    scene.wheel(-30.);

    assert_eq!(scene.offset(2).y, px(expected));
}

/// Node 0 scrolls 100px along x over node 1, 300px wide, and node 2, which
/// starts 150px from the left.
fn scrolling_root_along_x() -> Host {
    let mut host = Host::default();
    let root = add_div(&mut host, None, 100.0, false);
    host.tree.set_style(root, "overflow_x", "scroll").unwrap();
    let wide = add_div(&mut host, Some(root), 10.0, false);
    host.tree.set_style(wide, "width", 300.0).unwrap();
    let target = add_div(&mut host, Some(root), 10.0, false);
    host.tree.set_style(target, "width", 20.0).unwrap();
    host.tree.set_style(target, "margin_left", 150.0).unwrap();
    host
}

#[gpui::test]
fn a_prevented_listeners_reveal_along_x_stays(cx: &mut TestAppContext) {
    let js = on_wheel(
        0,
        r"
        (e) => {
            e.preventDefault();
            __inca_native__.scrollIntoViewNode(2, 'nearest', 'start');
        }
        ",
    );
    let mut scene = Scene::new(cx, scrolling_root_along_x(), &js);

    scene.wheel(-30.);

    assert_eq!(scene.offset(0).x, px(-150.));
}

#[gpui_cases]
#[gpui::test]
#[case::prevented("e.preventDefault();", -20.0)]
#[case::unprevented("", -50.0)]
fn a_capture_listeners_write_to_the_inner_container_keeps_the_write(
    cx: &mut TestAppContext,
    prevent: &str,
    expected: f32,
) {
    let js = listen(
        2,
        "wheel",
        &format!(
            r"
            (e) => {{
                {prevent}
                __inca_native__.scrollNode(2, null, 20);
            }}
            "
        ),
        true,
        false,
    );
    let mut scene = Scene::new(cx, scrollers_below_a_plain_root(), &js);

    scene.wheel(-30.);

    assert_eq!(scene.offset(2).y, px(expected));
    assert_eq!(scene.offset(1).y, px(0.));
}

#[gpui::test]
fn a_prevented_scroll_node_call_without_an_axis_leaves_the_wheel_step_undone(
    cx: &mut TestAppContext,
) {
    let js = on_wheel(
        0,
        r"
        (e) => {
            e.preventDefault();
            __inca_native__.scrollNode(2, null, null);
        }
        ",
    );
    let mut scene = Scene::new(cx, scrollers_below_a_plain_root(), &js);

    scene.wheel(-30.);

    assert_eq!(scene.offset(2).y, px(0.));
}
