// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[gpui::test]
fn a_script_offset_set_before_the_first_draw_is_applied(cx: &mut TestAppContext) {
    let scene = Scene::with_bindings(
        cx,
        one_container(),
        "",
        "__inca_native__.scrollNode(0, null, 40)",
    );
    assert_eq!(scene.offset(0).y, px(-40.));
    assert_eq!(scroll_of(&scene, 0), [0., 40., 0., 200.]);
}

#[gpui::test]
fn a_hidden_container_takes_a_script_offset(cx: &mut TestAppContext) {
    let scene = Scene::with_bindings(
        cx,
        styled_container(&[("overflow", "hidden")]),
        "",
        "__inca_native__.scrollNode(0, 30, 40)",
    );
    assert_eq!(scene.offset(0), point(px(-30.), px(-40.)));
    assert_eq!(scroll_of(&scene, 0), [30., 40., 200., 200.]);
}

#[gpui::test]
fn the_getter_clamps_to_the_last_frames_range(cx: &mut TestAppContext) {
    let scene = Scene::new(cx, one_container(), "");
    scene.js("__inca_native__.scrollNode(0, null, 999)");
    assert_eq!(scroll_of(&scene, 0), [0., 200., 0., 200.]);
}

#[gpui::test]
fn a_node_with_visible_overflow_ignores_a_script_offset(cx: &mut TestAppContext) {
    let scene = Scene::with_bindings(
        cx,
        styled_container(&[]),
        "",
        "__inca_native__.scrollNode(0, 30, 40)",
    );
    assert_eq!(scene.offset(0), point(px(0.), px(0.)));
    assert!(
        scene
            .engine
            .eval::<bool>("__inca_native__.scrollOf(0) === null")
            .unwrap()
    );
}

#[gpui::test]
fn a_wheel_scroll_fires_scroll_once_at_the_container(cx: &mut TestAppContext) {
    let record =
        "0: (e) => { ran.push('scroll:' + e.target + ':' + e.currentTarget + ':' + e.bubbles); }";
    let mut scene = Scene::new(cx, listened_container(), record);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["scroll:1:1:false"]"#);
}

#[gpui::test]
fn a_script_offset_before_the_first_draw_fires_once_after_it(cx: &mut TestAppContext) {
    let mut scene = Scene::with_bindings(
        cx,
        listened_container(),
        RECORD_SCROLL,
        "__inca_native__.scrollNode(1, null, 40)",
    );
    scene.frame();
    scene.frame();
    assert_eq!(scene.ran(), r#"["scroll:1:false"]"#);
}

#[gpui::test]
fn removing_the_listener_between_frames_fires_nothing(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, listened_container(), RECORD_SCROLL);
    scene.wheel(-50.);
    assert!(scene.host.borrow_mut().listeners.unregister(1, "scroll", 0));
    scene.wheel(-20.);
    assert_eq!(scene.ran(), r#"["scroll:1:false"]"#);
}

#[gpui::test]
fn destroying_the_container_between_frames_fires_nothing(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, listened_container(), RECORD_SCROLL);
    scene.wheel(-50.);
    let stale = scene.dispatcher.scroll_handle(1).unwrap();
    scene.js("__inca_native__.destroyNode(1)");
    stale.set_offset(point(px(0.), px(-30.)));
    scene.frame();
    assert_eq!(scene.ran(), r#"["scroll:1:false"]"#);
}

#[gpui::test]
fn a_script_write_fires_scroll_once(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, listened_container(), RECORD_SCROLL);
    scene.js("__inca_native__.scrollNode(1, null, 40)");
    scene.cx.update(|window, _| window.refresh());
    scene.cx.run_until_parked();
    scene.frame();
    scene.frame();
    assert_eq!(scene.ran(), r#"["scroll:1:false"]"#);
}

#[gpui::test]
fn no_scroll_fires_while_the_offset_is_unchanged(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, listened_container(), RECORD_SCROLL);
    scene.js("__inca_native__.scrollNode(1, null, 50)");
    scene.frame();
    scene.js("__inca_native__.scrollNode(1, null, 50)");
    scene.frame();
    scene.frame();
    assert_eq!(scene.ran(), r#"["scroll:1:false"]"#);
}

#[gpui::test]
fn a_scroll_past_the_end_fires_for_the_clamped_offset_once(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, listened_container(), RECORD_SCROLL);
    scene.js("__inca_native__.scrollNode(1, null, 999)");
    scene.frame();
    scene.frame();
    scene.js("__inca_native__.scrollNode(1, null, 200)");
    scene.frame();
    assert_eq!(scene.ran(), r#"["scroll:1:false"]"#);
}

#[gpui::test]
fn a_listener_added_later_sees_only_the_scrolling_after_it_was_added(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, one_container(), RECORD_SCROLL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), "[]");
    scene.host.borrow_mut().listeners.register(0, "scroll", 0);
    scene.frame();
    assert_eq!(
        scene.ran(),
        "[]",
        "the offset moved before the listener existed"
    );
    scene.wheel(-20.);
    assert_eq!(scene.ran(), r#"["scroll:0:false"]"#);
}

#[gpui::test]
fn an_ancestor_capture_listener_sees_a_listenerless_container_scroll_once(cx: &mut TestAppContext) {
    let mut host = parented_container();
    let root = 0;
    host.listeners.register_with(root, "scroll", 0, true, false);
    let record = "0: (e) => { ran.push('capture:' + e.target + ':' + e.eventPhase); }";
    let mut scene = Scene::new(cx, host, record);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["capture:1:1"]"#);
    scene.wheel(-20.);
    assert_eq!(scene.ran(), r#"["capture:1:1","capture:1:1"]"#);
}

#[gpui::test]
fn an_ancestor_bubble_listener_sees_nothing_of_a_listenerless_container_scroll(
    cx: &mut TestAppContext,
) {
    let mut host = parented_container();
    host.listeners.register(0, "scroll", 1);
    let mut scene = Scene::new(cx, host, RECORD_SCROLL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), "[]");
}

#[gpui::test]
fn a_scroll_no_listener_reaches_runs_no_js(cx: &mut TestAppContext) {
    let mut scene = Scene::new(cx, one_container(), "");
    // Counts each read of the callback table, which only a walk that
    // reaches a listener makes.
    scene.js(
        "globalThis.reads = 0; const table = globalThis.__inca_callbacks__; \
         Object.defineProperty(globalThis, '__inca_callbacks__', { configurable: true, \
         get() { globalThis.reads++; return table; } });",
    );
    scene.wheel(-50.);
    assert_eq!(scene.offset(0).y, px(-50.));
    assert_eq!(scene.engine.eval::<i32>("globalThis.reads").unwrap(), 0);
}

#[gpui::test]
fn scroll_stops_at_the_listening_container(cx: &mut TestAppContext) {
    let mut host = listened_container();
    host.listeners.register(0, "scroll", 1);
    let mut scene = Scene::new(cx, host, RECORD_SCROLL);
    scene.wheel(-50.);
    assert_eq!(scene.ran(), r#"["scroll:1:false"]"#);
}

#[gpui::test]
fn a_node_appended_in_the_same_tick_scrolls_into_view_once_it_has_a_box(cx: &mut TestAppContext) {
    let mut scene = Scene::with_bindings(cx, one_container(), "", "");
    scene.js(
        "const n = __inca_native__; const node = n.createNode('div'); \
         n.setStyle(node, 'width', 100); n.setStyle(node, 'height', 50); \
         n.appendChild(0, node); n.scrollIntoViewNode(node, 'start', 'nearest');",
    );
    assert_eq!(scene.offset(0).y, px(0.));

    scene.draw();
    scene.draw();

    assert_eq!(scene.offset(0).y, px(-250.));
}

#[gpui::test]
fn a_scroll_listener_run_requests_the_next_frame(cx: &mut TestAppContext) {
    let mut scene = Scene::with_bindings(
        cx,
        listened_container(),
        RECORD_SCROLL,
        "__inca_native__.scrollNode(1, null, 40)",
    );
    scene.frame();
    let frames = scene.cx.update(gpui::Window::simulate_next_frame);
    assert_eq!(scene.ran(), r#"["scroll:1:false"]"#);
    assert!(frames > 0);
}

#[gpui::test]
fn a_script_offset_then_a_reveal_in_the_same_tick_lands_on_the_item(cx: &mut TestAppContext) {
    let mut host = Host::default();
    let container = add_div(&mut host, None, 100.0, true);
    add_div(&mut host, Some(container), 200.0, false);
    let item = add_div(&mut host, Some(container), 50.0, false);
    add_div(&mut host, Some(container), 300.0, false);
    let scene = Scene::new(cx, host, "");
    scene.js(&format!(
        "__inca_native__.scrollNode(0, null, 100); \
         __inca_native__.scrollIntoViewNode({item}, 'start', 'nearest');"
    ));
    assert_eq!(scene.offset(0).y, px(-200.));
}

fn scroll_of(scene: &Scene, node: NodeId) -> Vec<f64> {
    scene
        .engine
        .eval(&format!("__inca_native__.scrollOf({node})"))
        .unwrap()
}

/// Callbacks 0 and 1 record `scroll` at the container and at its parent.
const RECORD_SCROLL: &str = "0: (e) => { ran.push('scroll:' + e.target + ':' + e.bubbles); }, \
                             1: () => { ran.push('parent'); }";

/// A scrolling node 1 under the plain root 0.
fn parented_container() -> Host {
    let mut host = Host::default();
    let root = add_div(&mut host, None, 100.0, false);
    let container = add_div(&mut host, Some(root), 100.0, true);
    add_div(&mut host, Some(container), 300.0, false);
    host
}

/// `parented_container` with a `scroll` listener on node 1.
fn listened_container() -> Host {
    let mut host = parented_container();
    host.listeners.register(1, "scroll", 0);
    host
}
