// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Facts about `gpui` that the event dispatch relies on, checked against
//! `gpui` itself on its test platform. A `gpui` upgrade that changes one of
//! them fails here first.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, DispatchPhase, FocusHandle, KeyBinding, KeyDownEvent, KeyUpEvent,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput,
    Point, ScrollDelta, ScrollHandle, ScrollWheelEvent, TestAppContext, VisualTestContext, Window,
    canvas, div, point, px,
};
use inca_test_macros::gpui_cases;

type Log = Rc<RefCell<Vec<String>>>;

fn say(log: &Log, text: &str) -> impl Fn(&mut Window, &mut App) + 'static + use<> {
    let (log, text) = (Rc::clone(log), text.to_owned());
    move |_, _| log.borrow_mut().push(text.clone())
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

type Build = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

struct Scene(Build);

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        (self.0)(window, cx)
    }
}

fn mount(
    cx: &mut TestAppContext,
    build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
) -> VisualTestContext {
    let window = cx.add_window(|_, _| Scene(Box::new(build)));
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    VisualTestContext::from_window(window.into(), cx)
}

fn at(x: f32, y: f32) -> Point<Pixels> {
    point(px(x), px(y))
}

fn press(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_mouse_down(at(x, y), MouseButton::Left, Modifiers::none());
}

/// Runs `listener` for the mouse button presses of the window in `phase`, from
/// a zero-size canvas.
fn press_tap(
    phase: DispatchPhase,
    listener: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            window.on_mouse_event(move |_: &MouseDownEvent, current, window, cx| {
                if current == phase {
                    listener(window, cx);
                }
            });
        },
    )
    .absolute()
    .size_0()
}

#[gpui::test]
fn gpui_runs_capture_listeners_in_paint_order_then_bubble_listeners_in_reverse(
    cx: &mut TestAppContext,
) {
    let log = Log::default();
    let l = Rc::clone(&log);
    let mut cx = mount(cx, move |_, _| {
        div()
            .size(px(100.))
            .capture_any_mouse_down({
                let say = say(&l, "capture root");
                move |_, window, cx| say(window, cx)
            })
            .on_any_mouse_down({
                let say = say(&l, "bubble root");
                move |_, window, cx| say(window, cx)
            })
            .child(
                div()
                    .size(px(50.))
                    .capture_any_mouse_down({
                        let say = say(&l, "capture child");
                        move |_, window, cx| say(window, cx)
                    })
                    .on_any_mouse_down({
                        let say = say(&l, "bubble child");
                        move |_, window, cx| say(window, cx)
                    }),
            )
            .into_any_element()
    });

    press(&mut cx, 10., 10.);

    assert_eq!(
        taken(&log),
        [
            "capture root",
            "capture child",
            "bubble child",
            "bubble root"
        ]
    );
}

/// `element` listening for a left press, a left release and a move in the
/// bubble phase, logging `name` for each.
fn listen_to_mouse(element: gpui::Div, log: &Log, name: &str) -> gpui::Div {
    let (down, up, moved) = (say(log, name), say(log, name), say(log, name));
    element
        .on_any_mouse_down(move |_, window, cx| down(window, cx))
        .on_mouse_up(MouseButton::Left, move |_, window, cx| up(window, cx))
        .on_mouse_move(move |_, window, cx| moved(window, cx))
}

#[gpui::test]
fn gpui_runs_a_first_child_canvas_bubble_listener_after_siblings_before_the_parent(
    cx: &mut TestAppContext,
) {
    let log = Log::default();
    let l = Rc::clone(&log);
    let mut cx = mount(cx, move |_, _| {
        let tap = {
            let (down, up, moved) = (say(&l, "tracker"), say(&l, "tracker"), say(&l, "tracker"));
            canvas(
                |_, _, _| (),
                move |_, (), window, _| {
                    window.on_mouse_event(move |_: &MouseDownEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            down(window, cx);
                        }
                    });
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            up(window, cx);
                        }
                    });
                    window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            moved(window, cx);
                        }
                    });
                },
            )
            .absolute()
            .size_0()
        };
        listen_to_mouse(div().size(px(100.)), &l, "root")
            .child(tap)
            .child(
                listen_to_mouse(div().size(px(50.)), &l, "sibling").child(listen_to_mouse(
                    div().size(px(20.)),
                    &l,
                    "grandchild",
                )),
            )
            .into_any_element()
    });

    let want = ["grandchild", "sibling", "tracker", "root"];
    press(&mut cx, 10., 10.);
    assert_eq!(taken(&log), want);
    cx.simulate_mouse_up(at(10., 10.), MouseButton::Left, Modifiers::none());
    assert_eq!(taken(&log), want);
    cx.simulate_mouse_move(at(11., 11.), None, Modifiers::none());
    assert_eq!(taken(&log), want);
}

#[gpui::test]
fn gpui_runs_only_the_containers_under_the_pointer_deepest_first(cx: &mut TestAppContext) {
    let log = Log::default();
    let l = Rc::clone(&log);
    let mut cx = mount(cx, move |_, _| {
        let listen = |name: &str| {
            let say = say(&l, name);
            move |_: &MouseDownEvent, window: &mut Window, cx: &mut App| say(window, cx)
        };
        div()
            .size(px(100.))
            .flex()
            .flex_row()
            .on_any_mouse_down(listen("root"))
            .child(
                div()
                    .size(px(50.))
                    .on_any_mouse_down(listen("left"))
                    .child(div().size(px(20.)).on_any_mouse_down(listen("left leaf"))),
            )
            .child(div().size(px(50.)).on_any_mouse_down(listen("right")))
            .into_any_element()
    });

    press(&mut cx, 10., 10.);
    assert_eq!(taken(&log), ["left leaf", "left", "root"]);
    press(&mut cx, 40., 40.);
    assert_eq!(taken(&log), ["left", "root"]);
    press(&mut cx, 70., 10.);
    assert_eq!(taken(&log), ["right", "root"]);
    press(&mut cx, 70., 80.);
    assert_eq!(taken(&log), ["root"]);
    press(&mut cx, 150., 150.);
    assert_eq!(taken(&log), Vec::<String>::new());
}

fn focusable_pair(parent: &FocusHandle, child: &FocusHandle) -> AnyElement {
    div()
        .id("parent")
        .track_focus(parent)
        .size(px(100.))
        .child(div().id("child").track_focus(child).size(px(50.)))
        .into_any_element()
}

fn handles(cx: &mut TestAppContext) -> (FocusHandle, FocusHandle) {
    cx.update(|cx| (cx.focus_handle(), cx.focus_handle()))
}

#[gpui::test]
fn gpui_skips_focus_on_press_when_mousedown_default_is_prevented(cx: &mut TestAppContext) {
    let (parent, child) = handles(cx);
    let prevent = Rc::new(Cell::new(true));
    let (p, c, flag) = (parent.clone(), child.clone(), Rc::clone(&prevent));
    let mut cx = mount(cx, move |_, _| {
        let flag = Rc::clone(&flag);
        div()
            .child(press_tap(DispatchPhase::Capture, move |window, _| {
                if flag.get() {
                    window.prevent_default();
                }
            }))
            .child(focusable_pair(&p, &c))
            .into_any_element()
    });
    let focused = |cx: &mut VisualTestContext| {
        cx.update(|window, _| (parent.is_focused(window), child.is_focused(window)))
    };

    press(&mut cx, 10., 10.);
    assert_eq!(focused(&mut cx), (false, false));

    prevent.set(false);
    press(&mut cx, 10., 10.);
    assert_eq!(focused(&mut cx), (false, true));

    press(&mut cx, 80., 80.);
    assert_eq!(focused(&mut cx), (true, false));
}

#[gpui::test]
fn gpui_lets_a_deferred_task_read_default_prevented_as_the_event_left_it(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(None));
    let s = Rc::clone(&seen);
    let mut cx = mount(cx, move |_, _| {
        let s = Rc::clone(&s);
        div()
            .size(px(100.))
            .child(press_tap(DispatchPhase::Bubble, move |window, cx| {
                window.prevent_default();
                let s = Rc::clone(&s);
                window.defer(cx, move |window, _| {
                    *s.borrow_mut() = Some(window.default_prevented());
                });
            }))
            .into_any_element()
    });

    press(&mut cx, 10., 10.);

    assert_eq!(*seen.borrow(), Some(true));
}

#[gpui::test]
fn gpui_clears_default_prevented_when_the_next_event_starts(cx: &mut TestAppContext) {
    let mut cx = mount(cx, |_, _| {
        div()
            .size(px(100.))
            .child(press_tap(DispatchPhase::Capture, |window, _| {
                window.prevent_default();
            }))
            .into_any_element()
    });
    press(&mut cx, 10., 10.);
    assert!(cx.update(|window, _| window.default_prevented()));

    cx.simulate_mouse_move(at(20., 20.), None, Modifiers::none());

    assert!(!cx.update(|window, _| window.default_prevented()));
}

#[gpui::test]
fn gpui_applies_the_wheel_scroll_step_in_the_bubble_phase_and_keeps_propagating(
    cx: &mut TestAppContext,
) {
    let log = Log::default();
    let handle = ScrollHandle::new();
    let (l, h) = (Rc::clone(&log), handle.clone());
    let mut cx = mount(cx, move |_, _| {
        let (root_log, root_handle) = (Rc::clone(&l), h.clone());
        let (tap_log, tap_handle) = (Rc::clone(&l), h.clone());
        div()
            .size(px(100.))
            .on_scroll_wheel(move |_, _, _| {
                let offset = f32::from(root_handle.offset().y);
                root_log.borrow_mut().push(format!("root {offset}"));
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, (), window, _| {
                        window.on_mouse_event(move |_: &ScrollWheelEvent, phase, _, _| {
                            let name = match phase {
                                DispatchPhase::Capture => "tracker capture",
                                DispatchPhase::Bubble => "tracker bubble",
                            };
                            let offset = f32::from(tap_handle.offset().y);
                            tap_log.borrow_mut().push(format!("{name} {offset}"));
                        });
                    },
                )
                .absolute()
                .size_0(),
            )
            .child(
                div()
                    .id("scroller")
                    .size(px(100.))
                    .overflow_y_scroll()
                    .track_scroll(&h)
                    .child(div().w(px(100.)).h(px(300.))),
            )
            .into_any_element()
    });

    cx.simulate_event(ScrollWheelEvent {
        position: at(10., 10.),
        delta: ScrollDelta::Pixels(point(px(0.), px(-20.))),
        ..Default::default()
    });

    assert_eq!(
        taken(&log),
        ["tracker capture 0", "tracker bubble -20", "root -20"]
    );
    assert_eq!(f32::from(handle.offset().y), -20.);
}

/// Mounts a 100px tracked container with hidden y overflow over 300px of
/// content.
fn mount_hidden_scroller(cx: &mut TestAppContext, handle: &ScrollHandle) -> VisualTestContext {
    let h = handle.clone();
    mount(cx, move |_, _| {
        div()
            .id("scroller")
            .size(px(100.))
            .overflow_y_hidden()
            .track_scroll(&h)
            .child(div().w(px(100.)).h(px(300.)))
            .into_any_element()
    })
}

#[gpui::test]
fn gpui_never_scrolls_a_tracked_container_with_hidden_overflow(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let mut cx = mount_hidden_scroller(cx, &handle);

    cx.simulate_event(ScrollWheelEvent {
        position: at(10., 10.),
        delta: ScrollDelta::Pixels(point(px(0.), px(-20.))),
        ..Default::default()
    });

    assert_eq!(f32::from(handle.offset().y), 0.);
}

#[gpui::test]
fn gpui_clamps_a_tracked_container_with_hidden_overflow_to_its_content(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let mut cx = mount_hidden_scroller(cx, &handle);
    handle.set_offset(point(px(0.), px(-1000.)));

    cx.update(|window, cx| window.draw(cx).clear(cx));

    assert_eq!(f32::from(handle.offset().y), -200.);
    assert_eq!(f32::from(handle.max_offset().y), 200.);
}

/// Mounts a window whose root and a focused child both log their capture
/// and bubble key listeners, then returns it with `root`'s extra builder
/// applied.
fn key_scene(
    cx: &mut TestAppContext,
    log: &Log,
    root: impl Fn(gpui::Div) -> gpui::Div + 'static,
) -> VisualTestContext {
    let child = cx.update(|cx| cx.focus_handle());
    let (l, c) = (Rc::clone(log), child.clone());
    let mut cx = mount(cx, move |_, _| {
        let down = |name: &str| {
            let say = say(&l, name);
            move |_: &KeyDownEvent, window: &mut Window, cx: &mut App| say(window, cx)
        };
        let up = |name: &str| {
            let say = say(&l, name);
            move |_: &KeyUpEvent, window: &mut Window, cx: &mut App| say(window, cx)
        };
        root(
            div()
                .size(px(100.))
                .capture_key_down(down("capture root"))
                .on_key_down(down("bubble root"))
                .capture_key_up(up("capture root"))
                .on_key_up(up("bubble root")),
        )
        .child(
            div()
                .id("child")
                .track_focus(&c)
                .size(px(50.))
                .capture_key_down(down("capture child"))
                .on_key_down(down("bubble child"))
                .capture_key_up(up("capture child"))
                .on_key_up(up("bubble child")),
        )
        .into_any_element()
    });
    cx.update(|window, cx| window.focus(&child, cx));
    cx
}

fn key_event(cx: &mut VisualTestContext, up: bool, key: &str) -> gpui::DispatchEventResult {
    let keystroke = gpui::Keystroke::parse(key).unwrap();
    let event = if up {
        PlatformInput::KeyUp(KeyUpEvent { keystroke })
    } else {
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        })
    };
    cx.update(|window, cx| window.dispatch_event(event, cx))
}

const KEY_ORDER: [&str; 4] = [
    "capture root",
    "capture child",
    "bubble child",
    "bubble root",
];

#[gpui_cases]
#[gpui::test]
#[case::key_down(false)]
#[case::key_up(true)]
fn gpui_runs_capture_key_listeners_from_the_root_down_before_the_bubble_ones(
    cx: &mut TestAppContext,
    up: bool,
) {
    let log = Log::default();
    let mut cx = key_scene(cx, &log, |root| root);

    let result = key_event(&mut cx, up, "x");

    assert_eq!(taken(&log), KEY_ORDER);
    assert!(result.propagate && !result.default_prevented);
}

#[gpui_cases]
#[gpui::test]
#[case::key_down(false)]
#[case::key_up(true)]
fn gpui_stops_later_key_listeners_and_reports_no_propagation_after_a_capture_stop(
    cx: &mut TestAppContext,
    up: bool,
) {
    let log = Log::default();
    let mut cx = key_scene(cx, &log, |root| {
        root.capture_key_down(|_, _, cx| cx.stop_propagation())
            .capture_key_up(|_, _, cx| cx.stop_propagation())
    });

    let result = key_event(&mut cx, up, "x");

    assert_eq!(taken(&log), ["capture root"]);
    assert!(!result.propagate && !result.default_prevented);
}

#[gpui_cases]
#[gpui::test]
#[case::key_down(false)]
#[case::key_up(true)]
fn gpui_reports_a_key_as_default_prevented_when_a_capture_key_listener_prevents_it(
    cx: &mut TestAppContext,
    up: bool,
) {
    let log = Log::default();
    let mut cx = key_scene(cx, &log, |root| {
        root.capture_key_down(|_, window, _| window.prevent_default())
            .capture_key_up(|_, window, _| window.prevent_default())
    });

    let result = key_event(&mut cx, up, "x");

    assert_eq!(taken(&log), KEY_ORDER);
    assert!(result.propagate && result.default_prevented);
}

#[gpui::test]
fn gpui_skips_the_key_listeners_of_a_self_tracked_root_when_nothing_is_focused(
    cx: &mut TestAppContext,
) {
    let log = Log::default();
    let handle = cx.update(|cx| cx.focus_handle());
    let (l, h) = (Rc::clone(&log), handle.clone());
    let mut cx = mount(cx, move |_, _| {
        let say = say(&l, "root");
        div()
            .id("root")
            .track_focus(&h)
            .size(px(100.))
            .capture_key_down(move |_, window, cx| say(window, cx))
            .into_any_element()
    });
    cx.update(|window, cx| window.focus(&handle, cx));

    key_event(&mut cx, false, "x");
    assert_eq!(taken(&log), ["root"]);

    cx.update(gpui::Window::blur);
    key_event(&mut cx, false, "x");
    assert_eq!(taken(&log), Vec::<String>::new());
}

gpui::actions!(inca_gpui_pins, [Pin]);

#[gpui::test]
fn gpui_runs_a_keybinding_and_its_action_before_any_key_listener(cx: &mut TestAppContext) {
    let log = Log::default();
    let acted = Rc::new(RefCell::new(0));
    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new("x", Pin, None)]);
        let acted = Rc::clone(&acted);
        cx.on_action(move |_: &Pin, _| *acted.borrow_mut() += 1);
    });
    let mut cx = key_scene(cx, &log, |root| root);

    let result = key_event(&mut cx, false, "x");

    assert_eq!(*acted.borrow(), 1);
    assert_eq!(taken(&log), Vec::<String>::new());
    assert!(!result.propagate);
}

#[gpui::test]
fn gpui_runs_an_on_hover_listener_in_the_mouse_move_dispatch_before_a_child_canvas(
    cx: &mut TestAppContext,
) {
    let log = Log::default();
    let l = Rc::clone(&log);
    let mut cx = mount(cx, move |_, _| {
        let (hover_log, move_log) = (Rc::clone(&l), Rc::clone(&l));
        div()
            .size(px(100.))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, (), window, _| {
                        window.on_mouse_event(move |_: &MouseMoveEvent, phase, _, _| {
                            if phase == DispatchPhase::Bubble {
                                move_log.borrow_mut().push("tracker move".to_owned());
                            }
                        });
                    },
                )
                .absolute()
                .size_0(),
            )
            .child(
                div()
                    .id("hovered")
                    .size(px(50.))
                    .on_hover(move |hovered, _, _| {
                        hover_log.borrow_mut().push(format!("hover {hovered}"));
                    }),
            )
            .into_any_element()
    });
    cx.simulate_mouse_move(at(200., 200.), None, Modifiers::none());
    taken(&log);

    cx.simulate_mouse_move(at(10., 10.), None, Modifiers::none());
    assert_eq!(taken(&log), ["hover true", "tracker move"]);
    cx.simulate_mouse_move(at(200., 200.), None, Modifiers::none());
    assert_eq!(taken(&log), ["hover false", "tracker move"]);
}

#[gpui::test]
fn gpui_gives_a_root_on_modifiers_changed_listener_every_modifier_change(cx: &mut TestAppContext) {
    let log = Log::default();
    let (root, child) = cx.update(|cx| (cx.focus_handle(), cx.focus_handle()));
    let (l, r, c) = (Rc::clone(&log), root.clone(), child.clone());
    let mut cx = mount(cx, move |_, _| {
        let l = Rc::clone(&l);
        div()
            .id("root")
            .track_focus(&r)
            .size(px(100.))
            .on_modifiers_changed(move |event, _, _| {
                let m = event.modifiers;
                l.borrow_mut().push(format!(
                    "{}{}{}",
                    u8::from(m.shift),
                    u8::from(m.control),
                    u8::from(m.alt)
                ));
            })
            .child(div().id("child").track_focus(&c).size(px(50.)))
            .into_any_element()
    });
    cx.update(|window, cx| window.focus(&child, cx));

    cx.simulate_modifiers_change(Modifiers::shift());
    cx.simulate_modifiers_change(Modifiers {
        shift: true,
        control: true,
        ..Modifiers::none()
    });
    cx.simulate_modifiers_change(Modifiers::none());

    assert_eq!(taken(&log), ["100", "110", "000"]);
}

#[gpui::test]
fn gpui_clamps_a_scroll_offset_to_the_content_on_the_next_draw(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let h = handle.clone();
    let mut cx = mount(cx, move |_, _| {
        div()
            .id("scroller")
            .size(px(100.))
            .overflow_y_scroll()
            .track_scroll(&h)
            .child(div().w(px(100.)).h(px(300.)))
            .into_any_element()
    });
    handle.set_offset(point(px(30.), px(-500.)));
    assert_eq!(f32::from(handle.offset().y), -500.);

    cx.update(|window, cx| window.draw(cx).clear(cx));

    assert_eq!(f32::from(handle.offset().y), -200.);
    assert_eq!(f32::from(handle.offset().x), 0.);
}

#[gpui::test]
fn gpui_redraws_after_a_focus_change_and_after_a_blur(cx: &mut TestAppContext) {
    let handle = cx.update(|cx| cx.focus_handle());
    let (renders, h) = (Rc::new(Cell::new(0_u32)), handle.clone());
    let counted = Rc::clone(&renders);
    let mut cx = mount(cx, move |_, _| {
        counted.set(counted.get() + 1);
        div()
            .id("tracked")
            .track_focus(&h)
            .size(px(50.))
            .into_any_element()
    });
    cx.run_until_parked();
    let renders_after = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        renders.get()
    };
    let start = renders_after(&mut cx);

    cx.update(|window, cx| window.focus(&handle, cx));
    let focused = renders_after(&mut cx);
    assert!(focused > start, "a focus change redraws");

    cx.update(|window, cx| window.focus(&handle, cx));
    assert_eq!(
        renders_after(&mut cx),
        focused,
        "the same focus redraws nothing"
    );

    cx.update(gpui::Window::blur);
    assert!(renders_after(&mut cx) > focused, "a blur redraws");
}
