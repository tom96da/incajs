// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

/// The `eventId` of the event `install_click_counter` saw last, 0 if none.
fn last_event_id(engine: &Engine) -> u64 {
    engine
        .eval::<u64>("globalThis.lastEvent?.eventId ?? 0")
        .unwrap()
}

/// `on_hover` covers both `mouseenter` and `mouseleave` with one GPUI
/// registration. Moving the pointer in, then out, must dispatch each
/// exactly once, in order.
#[gpui::test]
fn hover_dispatches_enter_then_leave(cx: &mut TestAppContext) {
    let host = Rc::new(RefCell::new(Host::default()));
    let node = {
        let mut host = host.borrow_mut();
        let node = host.tree.create_node("div").unwrap();
        host.tree.set_style(node, "width", 100.0).unwrap();
        host.tree.set_style(node, "height", 100.0).unwrap();
        node
    };
    let engine = test_support::engine_for(&host);
    engine
        .eval::<()>(&format!(
            r"
            globalThis.seen = [];
            const record = (event) => {{ globalThis.seen.push(event.type); }};
            __inca_test_listen__({node}, 'mouseenter', record);
            __inca_test_listen__({node}, 'mouseleave', record);
            "
        ))
        .unwrap();
    let mut cx = mount_clickable(cx, &host, &engine, node);

    // The window starts with the mouse at its default position, which lands
    // inside this 100x100 node, so mounting alone already fired a real
    // "mouseenter". Assert that before discarding it, so this test proves
    // the reset below is throwing away a real event, not a no-op.
    assert_ne!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        "[]",
        "mounting already hovered must have dispatched mouseenter by itself"
    );
    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.simulate_mouse_move(
        point(px(10.0), px(10.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        r#"["mouseenter","mouseleave"]"#
    );
}

/// A `mouseenter` carries the same mouse fields as
/// `mousedown`, built from the pointer's current position. `on_hover` hands
/// back only a `bool`.
#[gpui::test]
fn mouseenter_carries_the_mouse_fields(cx: &mut TestAppContext) {
    let host = Rc::new(RefCell::new(Host::default()));
    let node = {
        let mut host = host.borrow_mut();
        let node = host.tree.create_node("div").unwrap();
        host.tree.set_style(node, "width", 100.0).unwrap();
        host.tree.set_style(node, "height", 100.0).unwrap();
        node
    };
    let engine = test_support::engine_for(&host);
    install_click_counter(&engine, node, "mouseenter");
    let mut cx = mount_clickable(cx, &host, &engine, node);

    // Settle the implicit initial hover (the window's default mouse
    // position lands inside this 100x100 node) before the transition the
    // assertions below are about.
    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    let previous = last_event_id(&engine);
    engine.eval::<()>("globalThis.clicks = 0;").unwrap();

    cx.simulate_mouse_move(
        point(px(10.0), px(20.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    assert_eq!(clicks(&engine), 1.0);
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify({ ...globalThis.lastEvent, eventId: undefined, timeStamp: undefined })")
            .unwrap(),
        format!(
            concat!(
                r#"{{"type":"mouseenter","target":{node},"currentTarget":null,"#,
                r#""bubbles":false,"cancelable":false,"composed":false,"#,
                r#""eventPhase":0,"isTrusted":true,"#,
                r#""clientX":10,"clientY":20,"screenX":10,"screenY":20,"offsetX":10,"offsetY":20,"x":10,"y":20,"relatedTarget":null,"pageX":10,"pageY":20,"#,
                r#""movementX":0,"movementY":0,"button":0,"buttons":0,"detail":0,"#,
                r#""ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false}}"#
            ),
            node = node
        )
    );
    assert!(last_event_id(&engine) > previous);
}

/// A node listening only for `mouseleave` doesn't spuriously fire it on
/// mount just because the pointer already sits over it. The hover state
/// starts empty, so the mount-time check can only ever produce an enter
/// transition, never a leave one, regardless of which names JS registered.
#[gpui::test]
fn mounting_already_hovered_does_not_fire_a_spurious_leave(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    let engine = test_support::engine_for(&host);
    install_click_counter(&engine, node, "mouseleave");
    let _cx = mount_clickable(cx, &host, &engine, node);

    assert_eq!(
        clicks(&engine),
        0.0,
        "a mouseleave-only listener must not fire just from mounting already hovered"
    );
}

/// `buttons` reports every button held. Pressing a second button while the
/// first is still down unions its bit in, and releasing one button clears
/// only that bit.
#[gpui::test]
fn buttons_tracks_every_button_currently_held(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    let engine = test_support::engine_for(&host);
    for event in ["mousedown", "mouseup", "mousemove"] {
        install_click_counter(&engine, node, event);
    }
    let mut cx = mount_clickable(cx, &host, &engine, node);

    let buttons = || -> u8 { engine.eval("globalThis.lastEvent.buttons").unwrap() };
    let point = point(px(10.0), px(10.0));

    cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(buttons(), 0b0_0001, "the just-pressed button alone");

    cx.simulate_mouse_down(point, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(buttons(), 0b0_0011, "both buttons held at once");

    cx.simulate_mouse_move(point, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        buttons(),
        0b0_0011,
        "a move reports every button still held"
    );

    cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        buttons(),
        0b0_0010,
        "releasing one button clears only its bit"
    );
}

/// A release that no listener hears still clears its button, so a later
/// `mousemove` reports nothing held.
#[gpui::test]
fn buttons_clears_after_a_release_nobody_listens_to(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    let engine = test_support::engine_for(&host);
    for event in ["mousedown", "mousemove"] {
        install_click_counter(&engine, node, event);
    }
    let mut cx = mount_clickable(cx, &host, &engine, node);
    let point = point(px(10.0), px(10.0));
    let buttons = || -> u8 { engine.eval("globalThis.lastEvent.buttons").unwrap() };

    cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::none());
    assert_eq!(buttons(), 0b0_0001);
    cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point, None::<MouseButton>, Modifiers::none());

    assert_eq!(
        engine.eval::<String>("lastEvent.type").unwrap(),
        "mousemove"
    );
    assert_eq!(buttons(), 0);
}

/// `movementX`/`movementY` are a delta from the last pointer move, measured
/// from the pointer's own previous position.
#[gpui::test]
fn movement_tracks_the_delta_from_the_last_pointer_move(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    let engine = test_support::engine_for(&host);
    install_click_counter(&engine, node, "mousemove");
    let mut cx = mount_clickable(cx, &host, &engine, node);

    let movement = || -> (f32, f32) {
        (
            engine.eval("globalThis.lastEvent.movementX").unwrap(),
            engine.eval("globalThis.lastEvent.movementY").unwrap(),
        )
    };

    cx.simulate_mouse_move(
        point(px(10.0), px(10.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert_eq!(movement(), (0.0, 0.0), "nothing to take a delta from yet");

    cx.simulate_mouse_move(
        point(px(30.0), px(5.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert_eq!(movement(), (20.0, -5.0));
}

/// One `mousemove` bubbling from a child to its listening parent reports
/// the same `movementX`/`movementY` to both.
#[gpui::test]
fn bubbled_mousemove_gives_every_listener_the_same_movement(cx: &mut TestAppContext) {
    let (host, parent) = build_clickable_tree();
    let child = {
        let mut host = host.borrow_mut();
        let child = host.tree.create_node("div").unwrap();
        host.tree.set_style(child, "width", 100.0).unwrap();
        host.tree.set_style(child, "height", 100.0).unwrap();
        host.tree.append_child(parent, child).unwrap();
        child
    };
    let engine = test_support::engine_for(&host);
    engine
        .eval::<()>(&format!(
            r"
            globalThis.seen = [];
            const record = (e) => {{
                globalThis.seen.push([e.target, e.movementX, e.movementY]);
            }};
            __inca_test_listen__({parent}, 'mousemove', record);
            __inca_test_listen__({child}, 'mousemove', record);
            "
        ))
        .unwrap();
    let mut cx = mount_clickable(cx, &host, &engine, parent);

    for (x, y) in [(10.0, 10.0), (30.0, 5.0)] {
        cx.simulate_mouse_move(point(px(x), px(y)), None::<MouseButton>, Modifiers::none());
    }
    cx.run_until_parked();

    let seen: Vec<(u32, f32, f32)> =
        serde_json::from_str(&engine.eval::<String>("JSON.stringify(seen)").unwrap()).unwrap();
    assert_eq!(seen.len(), 4, "each move reaches the child then the parent");
    assert!(seen.iter().all(|&(target, ..)| target == child));
    assert_eq!((seen[0].1, seen[0].2), (seen[1].1, seen[1].2));
    assert_eq!((seen[2].1, seen[2].2), (20.0, -5.0));
    assert_eq!((seen[3].1, seen[3].2), (20.0, -5.0));
}

/// An inert child never becomes the target, so the listening parent does.
#[gpui::test]
fn an_inert_child_is_never_the_event_target(cx: &mut TestAppContext) {
    let (host, parent) = build_clickable_tree();
    let child = {
        let mut host = host.borrow_mut();
        let child = host.tree.create_node("div").unwrap();
        host.tree.set_style(child, "width", 100.0).unwrap();
        host.tree.set_style(child, "height", 100.0).unwrap();
        host.tree.set_attribute(child, "inert", true).unwrap();
        host.tree.append_child(parent, child).unwrap();
        child
    };
    let engine = test_support::engine_for(&host);
    engine
        .eval::<()>(&format!(
            r"
            globalThis.seen = [];
            const record = (e) => {{ globalThis.seen.push(e.target); }};
            __inca_test_listen__({parent}, 'mousemove', record);
            __inca_test_listen__({child}, 'mousemove', record);
            "
        ))
        .unwrap();
    let mut cx = mount_clickable(cx, &host, &engine, parent);

    cx.simulate_mouse_move(
        point(px(10.0), px(10.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.run_until_parked();

    let seen: Vec<u32> =
        serde_json::from_str(&engine.eval::<String>("JSON.stringify(seen)").unwrap()).unwrap();
    assert_eq!(seen, vec![parent]);
}

/// A second genuine `mousemove` at the same position reports no movement,
/// even though the first one at that position did.
#[gpui::test]
fn repeated_mousemove_at_one_position_reports_zero_movement(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    let engine = test_support::engine_for(&host);
    install_click_counter(&engine, node, "mousemove");
    let mut cx = mount_clickable(cx, &host, &engine, node);

    let mut movements = Vec::new();
    for x in [5.0, 10.0, 10.0] {
        cx.simulate_mouse_move(
            point(px(x), px(10.0)),
            None::<MouseButton>,
            Modifiers::none(),
        );
        cx.run_until_parked();
        movements.push(
            engine
                .eval::<f32>("globalThis.lastEvent.movementX")
                .unwrap(),
        );
    }

    assert_eq!(movements, [0.0, 5.0, 0.0]);
}
