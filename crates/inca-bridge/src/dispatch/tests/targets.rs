// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn a_click_targets_the_nearest_common_ancestor_of_press_and_release() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    let other = {
        let mut h = host.borrow_mut();
        let other = h.tree.create_node("div").unwrap();
        h.tree.append_child(parent, other).unwrap();
        other
    };
    assert_eq!(dispatcher.common_ancestor(child, other), Some(parent));
    assert_eq!(dispatcher.common_ancestor(child, parent), Some(parent));
    assert_eq!(dispatcher.common_ancestor(child, child), Some(child));
    assert_eq!(dispatcher.common_ancestor(child, outer), Some(outer));
}

#[gpui::test]
fn a_release_clicks_only_when_both_containers_share_an_ancestor(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, a] = tree_of_three(&host);
    let (b, apart) = {
        let mut h = host.borrow_mut();
        let b = h.tree.create_node("div").unwrap();
        h.tree.append_child(parent, b).unwrap();
        (b, h.tree.create_node("div").unwrap())
    };
    dispatcher.engine.eval::<()>(RECORDER).unwrap();
    for node in [parent, a, b, apart] {
        listen(&host, node, "click");
    }
    let release = gpui::MouseUpEvent {
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    };
    let cx = cx.add_empty_window();
    let mut clicked = |pressed, released| {
        cx.update(|window, cx| {
            dispatcher.pointer_released(&release, pressed, released, window, cx);
        });
        seen_rows(&dispatcher).len()
    };

    assert_eq!(clicked(Some(a), b), 1, "siblings share the parent");
    assert_eq!(clicked(Some(a), apart), 0, "no common ancestor");
    assert_eq!(clicked(None, a), 0, "no press container");

    host.borrow_mut().tree.destroy_node(b);
    assert_eq!(clicked(Some(a), b), 0, "the release node is gone");
    assert_eq!(clicked(Some(b), a), 0, "the press node is gone");
}

#[gpui::test]
fn a_removed_node_is_no_related_target_and_no_previous_container(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    {
        let mut host = host.borrow_mut();
        let root = host.root;
        host.tree.append_child(root, outer).unwrap();
    }
    dispatcher
        .engine
        .eval::<()>(
            "globalThis.seen = []; globalThis.__inca_callbacks__ = { 0: (e) => { \
             globalThis.seen.push([e.type, e.relatedTarget]); } };",
        )
        .unwrap();
    listen(&host, outer, "mouseover");
    listen(&host, outer, "mouseout");
    listen(&host, outer, "focus");
    let cx = cx.add_empty_window();
    let at = |related_target| {
        EventPayload::Mouse(MousePayload {
            related_target,
            ..MousePayload::at(gpui::Point::default(), gpui::Modifiers::none())
        })
    };

    cx.update(|window, cx| {
        dispatcher.fire(outer, "mouseover", &at(Some(parent)), window, cx);
        dispatcher.fire(outer, "mouseover", &at(Some(99)), window, cx);
        for related_target in [Some(parent), Some(99), None] {
            let payload = EventPayload::Focus { related_target };
            dispatcher.fire(outer, "focus", &payload, window, cx);
        }
    });
    assert_eq!(
        seen(&dispatcher),
        serde_json::json!([
            ["mouseover", parent],
            ["mouseover", null],
            ["focus", parent],
            ["focus", null],
            ["focus", null]
        ])
    );

    cx.update(|window, cx| dispatcher.report_hover(Some(child), window, cx));
    seen(&dispatcher);
    host.borrow_mut().tree.destroy_node(child);
    cx.update(|window, cx| dispatcher.report_hover(Some(parent), window, cx));
    assert_eq!(seen(&dispatcher), serde_json::json!([]));
    cx.update(|window, cx| dispatcher.report_hover(Some(outer), window, cx));
    assert_eq!(
        seen(&dispatcher),
        serde_json::json!([["mouseout", outer], ["mouseover", parent]])
    );

    // A detached deepest container gets no mouseout.
    cx.update(|window, cx| dispatcher.report_hover(Some(parent), window, cx));
    seen(&dispatcher);
    host.borrow_mut().tree.remove_child(outer, parent).unwrap();
    cx.update(|window, cx| dispatcher.report_hover(None, window, cx));
    assert_eq!(seen(&dispatcher), serde_json::json!([]));
}

fn record_box(host: &Rc<RefCell<Host>>, node: NodeId, left: f32, top: f32, border: f32) {
    host.borrow().layout.borrow_mut().insert(
        node,
        inca_gpui::NodeLayout {
            bounds: gpui::Bounds::new(point(px(left), px(top)), gpui::size(px(100.0), px(50.0))),
            border: px(border),
            scroll: gpui::Point::default(),
        },
    );
}

#[gpui::test]
fn offset_falls_back_to_the_client_position_when_unrecorded(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    listen(&host, parent, "click");
    callbacks(
        &dispatcher,
        "0: (e) => out.push([e.target, e.offsetX, e.offsetY, e.clientX, e.clientY])",
    );
    let click = EventPayload::Mouse(MousePayload::at(at(60.0, 40.0), gpui::Modifiers::none()));

    let out = fire_and_read(cx, &dispatcher, child, "click", &click);
    assert_eq!(out, serde_json::json!([[child, 60, 40, 60, 40]]));
}

#[gpui::test]
fn a_parent_listener_reads_the_targets_offset(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    listen(&host, parent, "click");
    callbacks(
        &dispatcher,
        "0: (e) => out.push([e.target, e.offsetX, e.offsetY, e.clientX, e.clientY])",
    );
    let click = EventPayload::Mouse(MousePayload::at(at(60.0, 40.0), gpui::Modifiers::none()));
    record_box(&host, parent, 500.0, 500.0, 9.0);
    record_box(&host, child, 10.0, 20.0, 2.0);
    let out = fire_and_read(cx, &dispatcher, child, "click", &click);
    assert_eq!(out, serde_json::json!([[child, 48, 18, 60, 40]]));
}

#[gpui::test]
fn hover_events_measure_the_offset_from_the_node_they_fire_at(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    listen(&host, parent, "mouseover");
    listen(&host, parent, "mouseenter");
    callbacks(
        &dispatcher,
        "0: (e) => out.push([e.type, e.target, e.offsetX, e.offsetY, e.clientX, e.clientY])",
    );
    record_box(&host, parent, 10.0, 20.0, 2.0);
    record_box(&host, child, 30.0, 40.0, 0.0);

    // The pointer is over `child`: `mouseover` fires there and bubbles to
    // `parent`, then `mouseenter` fires at `parent`.
    let cx = cx.add_empty_window();
    cx.update(|window, cx| dispatcher.report_hover(Some(child), window, cx));

    let out = read_out(&dispatcher);
    // The pointer is at (0,0) in the test window. Each event reads the box
    // of its own target.
    assert_eq!(
        out,
        serde_json::json!([
            ["mouseover", child, -30, -40, 0, 0],
            ["mouseenter", parent, -12, -22, 0, 0]
        ])
    );
}

#[gpui::test]
fn a_wheel_event_carries_the_offset(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, _, child] = tree_of_three(&host);
    listen(&host, child, "wheel");
    callbacks(&dispatcher, "0: (e) => out.push([e.offsetX, e.offsetY])");
    record_box(&host, child, 10.0, 20.0, 0.0);

    let out = fire_and_read(cx, &dispatcher, child, "wheel", &wheel_at(60.0, 40.0));

    assert_eq!(out, serde_json::json!([[50, 20]]));
}
