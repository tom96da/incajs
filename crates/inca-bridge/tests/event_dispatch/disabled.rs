// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use inca_test_macros::gpui_cases;

/// A 200x200 `root` holding a disabled 100x50 `button`, which holds a 100x50 `kid`. Every
/// listener records `[type, currentTarget]` into `globalThis.seen`.
struct Disabled {
    cx: VisualTestContext,
    engine: Rc<Engine>,
    host: Rc<RefCell<Host>>,
    root: NodeId,
    button: NodeId,
    kid: NodeId,
}

fn mount_disabled(cx: &mut TestAppContext, listening: &[(&str, &[&str])]) -> Disabled {
    let host = Rc::new(RefCell::new(Host::default()));
    let (root, button, kid) = {
        let mut host = host.borrow_mut();
        let root = host.tree.create_node("div").unwrap();
        host.tree.set_style(root, "width", 200.0).unwrap();
        host.tree.set_style(root, "height", 200.0).unwrap();
        let button = host.tree.create_node("button").unwrap();
        host.tree.set_style(button, "width", 100.0).unwrap();
        host.tree.set_style(button, "height", 50.0).unwrap();
        host.tree.set_attribute(button, "disabled", true).unwrap();
        host.tree.append_child(root, button).unwrap();
        let kid = host.tree.create_node("div").unwrap();
        host.tree.set_style(kid, "width", 100.0).unwrap();
        host.tree.set_style(kid, "height", 50.0).unwrap();
        host.tree.append_child(button, kid).unwrap();
        (root, button, kid)
    };
    let engine = test_support::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    for (name, events) in listening {
        let node = match *name {
            "root" => root,
            "button" => button,
            _ => kid,
        };
        for event in *events {
            engine
                .eval::<()>(&format!(
                    r"
                    __inca_test_listen__({node}, '{event}', (e) => {{
                        globalThis.seen.push([e.type, e.currentTarget]);
                    }});
                    "
                ))
                .unwrap();
        }
    }
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node: root,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    Disabled {
        cx: VisualTestContext::from_window(window.into(), cx),
        engine,
        host,
        root,
        button,
        kid,
    }
}

impl Disabled {
    fn press_and_release(&mut self, (x, y): (f32, f32), click_count: usize) {
        let position = point(px(x), px(y));
        self.cx.simulate_event(MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count,
            first_mouse: false,
        });
        self.cx.simulate_event(MouseUpEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count,
        });
    }

    fn seen(&self) -> Vec<(String, NodeId)> {
        let seen = self
            .engine
            .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
            .unwrap();
        serde_json::from_str(&seen).unwrap()
    }
}

#[gpui_cases]
#[gpui::test]
#[case::mousedown("mousedown", 1)]
#[case::mouseup("mouseup", 1)]
#[case::click("click", 1)]
#[case::dblclick("dblclick", 2)]
fn a_disabled_button_and_its_ancestors_get_no_pointer_event(
    cx: &mut TestAppContext,
    event: &str,
    click_count: usize,
) {
    let mut d = mount_disabled(cx, &[("button", &[event]), ("root", &[event])]);

    d.press_and_release((10.0, 10.0), click_count);
    let disabled = d.seen();
    d.host
        .borrow_mut()
        .tree
        .remove_attribute(d.button, "disabled")
        .unwrap();
    d.press_and_release((10.0, 10.0), click_count);

    assert_eq!(disabled, []);
    assert_eq!(
        d.seen(),
        [(event.to_owned(), d.button), (event.to_owned(), d.root)]
    );
}

#[gpui::test]
fn a_press_on_a_disabled_button_released_on_an_ancestor_is_dropped(cx: &mut TestAppContext) {
    let events: &[&str] = &["mouseup", "click"];
    let mut d = mount_disabled(cx, &[("button", events), ("root", events)]);
    let position = |x, y| point(px(x), px(y));

    d.cx.simulate_mouse_down(position(10.0, 10.0), MouseButton::Left, Modifiers::none());
    d.cx.simulate_mouse_up(position(150.0, 150.0), MouseButton::Left, Modifiers::none());

    assert_eq!(d.seen(), []);
}

#[gpui::test]
fn a_press_elsewhere_released_on_a_disabled_button_is_dropped(cx: &mut TestAppContext) {
    let events: &[&str] = &["mouseup", "click"];
    let mut d = mount_disabled(cx, &[("button", events), ("root", events)]);
    let position = |x, y| point(px(x), px(y));

    d.cx.simulate_mouse_down(position(150.0, 150.0), MouseButton::Left, Modifiers::none());
    d.cx.simulate_mouse_up(position(10.0, 10.0), MouseButton::Left, Modifiers::none());

    assert_eq!(d.seen(), []);
}

#[gpui_cases]
#[gpui::test]
#[case::contextmenu("contextmenu")]
#[case::auxclick("auxclick")]
fn a_right_press_reaches_the_ancestors_of_a_disabled_button(cx: &mut TestAppContext, event: &str) {
    let mut d = mount_disabled(cx, &[("root", &[event])]);
    let position = point(px(10.0), px(10.0));

    d.cx.simulate_mouse_down(position, MouseButton::Right, Modifiers::none());
    d.cx.simulate_mouse_up(position, MouseButton::Right, Modifiers::none());

    assert_eq!(d.seen(), [(event.to_owned(), d.root)]);
}

#[gpui::test]
fn a_disabled_button_keeps_its_hover_and_move_events(cx: &mut TestAppContext) {
    let events: &[&str] = &["mouseenter", "mousemove"];
    let mut d = mount_disabled(cx, &[("button", events), ("root", &["mousemove"])]);

    // The pointer starts inside the button; leave and clear what mounting fired.
    d.cx.simulate_mouse_move(point(px(250.0), px(250.0)), None, Modifiers::none());
    d.seen();
    d.cx.simulate_mouse_move(point(px(10.0), px(10.0)), None, Modifiers::none());

    let mut seen = d.seen();
    seen.sort();
    let mut want = vec![
        ("mouseenter".to_owned(), d.button),
        ("mousemove".to_owned(), d.button),
        ("mousemove".to_owned(), d.root),
    ];
    want.sort();
    assert_eq!(seen, want);
}

#[gpui::test]
fn a_disabled_button_keeps_a_listener_below_it(cx: &mut TestAppContext) {
    let events: &[&str] = &["click"];
    let mut d = mount_disabled(cx, &[("kid", events), ("button", events), ("root", events)]);

    d.press_and_release((10.0, 10.0), 1);

    assert_eq!(d.seen(), [("click".to_owned(), d.kid)]);
}
