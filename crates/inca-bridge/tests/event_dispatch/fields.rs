// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

/// A `mousedown`'s payload is DOM-`MouseEvent`-shaped: coordinates, which
/// button, and which buttons are held (just the one just pressed).
#[gpui::test]
fn mousedown_carries_dom_shaped_fields(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("mousedown");
    let engine = Rc::new(Engine::new().unwrap());
    install_click_counter(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_mouse_down(
        point(px(10.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert_eq!(clicks(&engine), 1.0);
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify({ ...globalThis.lastEvent, timeStamp: undefined })")
            .unwrap(),
        format!(
            concat!(
                r#"{{"type":"mousedown","target":{node},"currentTarget":null,"#,
                r#""bubbles":true,"cancelable":true,"composed":true,"#,
                r#""eventPhase":0,"isTrusted":true,"#,
                r#""clientX":10,"clientY":20,"screenX":10,"screenY":20,"offsetX":10,"offsetY":20,"x":10,"y":20,"relatedTarget":null,"pageX":10,"pageY":20,"#,
                r#""movementX":0,"movementY":0,"button":0,"buttons":1,"detail":1,"#,
                r#""ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false,"#,
                r#""eventId":1}}"#
            ),
            node = node
        )
    );
}

/// A `wheel`'s payload carries DOM-`WheelEvent`-shaped delta fields plus
/// every field a `mouse` payload does — `WheelEvent` extends `MouseEvent`.
#[gpui::test]
fn wheel_carries_dom_shaped_delta_fields(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("wheel");
    let engine = Rc::new(Engine::new().unwrap());
    install_click_counter(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-5.0))),
        ..Default::default()
    });

    assert_eq!(clicks(&engine), 1.0);
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify({ ...globalThis.lastEvent, timeStamp: undefined })")
            .unwrap(),
        format!(
            concat!(
                r#"{{"type":"wheel","target":{node},"currentTarget":null,"#,
                r#""bubbles":true,"cancelable":true,"composed":true,"#,
                r#""eventPhase":0,"isTrusted":true,"#,
                r#""clientX":10,"clientY":10,"screenX":10,"screenY":10,"offsetX":10,"offsetY":10,"x":10,"y":10,"relatedTarget":null,"pageX":10,"pageY":10,"#,
                r#""movementX":0,"movementY":0,"button":0,"buttons":0,"detail":0,"#,
                r#""ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false,"#,
                r#""deltaX":0,"deltaY":5,"deltaZ":0,"deltaMode":0,"#,
                r#""eventId":1}}"#
            ),
            node = node
        )
    );
}

/// A mouse button held while scrolling still shows up in a `wheel`'s
/// `buttons` — the `held_buttons` tracking `mousedown`/`mouseup` update is
/// shared with every payload kind, not copied per kind.
#[gpui::test]
fn wheel_reports_a_button_held_from_an_earlier_mousedown(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("mousedown");
    host.borrow_mut().listeners.register(node, "wheel", 0);
    let engine = Rc::new(Engine::new().unwrap());
    install_click_counter(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Middle,
        Modifiers::none(),
    );
    cx.run_until_parked();
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        ..Default::default()
    });

    let buttons: u8 = engine.eval("globalThis.lastEvent.buttons").unwrap();
    assert_eq!(buttons, 0b0_0100);
}

/// A `wheel` on a node nothing listens for must not call into JS — same
/// guarantee `no_listener_registered_reports_nothing` gives `click`.
#[gpui::test]
fn wheel_with_no_listener_reports_nothing(cx: &mut TestAppContext) {
    let host = Rc::new(RefCell::new(Host::default()));
    let node = {
        let mut host = host.borrow_mut();
        let node = host.tree.create_node("div").unwrap();
        host.tree.set_style(node, "width", 100.0).unwrap();
        host.tree.set_style(node, "height", 100.0).unwrap();
        node
    };
    let engine = Rc::new(Engine::new().unwrap());
    install_click_counter(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        ..Default::default()
    });

    assert_eq!(clicks(&engine), 0.0);
}
