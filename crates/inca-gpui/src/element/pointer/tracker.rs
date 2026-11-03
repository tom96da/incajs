// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reports each pointer input once, with the deepest container as its target.

use super::{
    App, ControlClick, DispatchPhase, EventPayload, EventSink, IntoElement, MouseDownEvent,
    MouseExitEvent, MouseMoveEvent, MouseTarget, MouseUpEvent, NodeId, RootStep, ScrollHandle,
    ScrollWheelEvent, Styled, WheelChain, Window, canvas, completes_click, control_click,
    mouse_target, next_offset, opens_context_menu, restored_down, restored_up, slot,
};

/// `event` with the click count of its button's recorded press, if any.
fn with_press_count(event: &MouseUpEvent, cx: &App) -> MouseUpEvent {
    let slot = slot(event.button);
    let mut event = event.clone();
    if let Some((_, count)) = cx.try_global::<MouseTarget>().and_then(|t| t.pressed[slot]) {
        event.click_count = count;
    }
    event
}

/// The deepest container that reported for the mouse event being handled,
/// else `root`.
fn target_or(cx: &App, root: NodeId) -> NodeId {
    mouse_target(cx).unwrap_or(root)
}

/// Reports the release of `event`, under `root` when no container is: its
/// `mouseup`, then the clicks it completes. Forgets that button's press.
fn report_release(
    sink: &impl EventSink,
    root: NodeId,
    event: &MouseUpEvent,
    window: &mut Window,
    cx: &mut App,
) {
    let slot = slot(event.button);
    let target = cx.default_global::<MouseTarget>();
    let released = target.current.unwrap_or(root);
    let pressed = target.pressed[slot].map(|(id, _)| id);
    let completes = completes_click(event.button, event.modifiers, control_click(cx).macos);
    let pressed = pressed.filter(|_| completes);
    let event = with_press_count(event, cx);
    sink.pointer_input("mouseup", released, &(&event).into(), window, cx);
    sink.pointer_released(&event, pressed, released, window, cx);
    cx.default_global::<MouseTarget>().pressed[slot] = None;
}

// Zero-size first child of the root. Its capture handlers run before any
// container's, and its bubble handlers run after every container's below the
// root, so each input is reported once, with its target settled.
pub(in crate::element) fn pointer_tracker<E: EventSink + Clone + 'static>(
    root: NodeId,
    dispatch: E,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            let moved = dispatch.clone();
            window.on_mouse_event(
                move |event: &MouseMoveEvent, phase, window, cx| match phase {
                    DispatchPhase::Capture => {
                        cx.default_global::<MouseTarget>().current = None;
                        cx.default_global::<ControlClick>().held = event.modifiers.control;
                        moved.pointer_moved(event.position, cx);
                    }
                    DispatchPhase::Bubble => {
                        let target = target_or(cx, root);
                        moved.pointer_input("mousemove", target, &event.into(), window, cx);
                    }
                },
            );
            let pressed = dispatch.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                let event = &restored_down(event, cx);
                match phase {
                    DispatchPhase::Capture => {
                        let slot = slot(event.button);
                        let target = cx.default_global::<MouseTarget>();
                        target.current = None;
                        target.pressed[slot] = None;
                        // gpui would focus before the listeners run; the sink focuses after.
                        window.prevent_default();
                    }
                    DispatchPhase::Bubble => {
                        let target = target_or(cx, root);
                        pressed.pointer_input("mousedown", target, &event.into(), window, cx);
                        if opens_context_menu(
                            event.button,
                            event.modifiers,
                            control_click(cx).macos,
                        ) {
                            let payload = EventPayload::context_menu(event);
                            pressed.fire(target, "contextmenu", &payload, window, cx);
                        }
                    }
                }
            });
            let released = dispatch.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| match phase {
                DispatchPhase::Capture => cx.default_global::<MouseTarget>().current = None,
                DispatchPhase::Bubble => {
                    let event = restored_up(event, cx);
                    report_release(&released, root, &event, window, cx);
                }
            });
            let scrolled = dispatch.clone();
            window.on_mouse_event(
                move |event: &ScrollWheelEvent, phase, window, cx| match phase {
                    DispatchPhase::Capture => {
                        cx.default_global::<MouseTarget>().current = None;
                        cx.default_global::<WheelChain>().0.clear();
                    }
                    DispatchPhase::Bubble => {
                        let mut chain = std::mem::take(&mut cx.default_global::<WheelChain>().0);
                        let delta = event.delta.pixel_delta(window.line_height());
                        let could_move = |(id, handle): &(NodeId, ScrollHandle)| {
                            scrolled
                                .wheel_axes(*id)
                                .and_then(|axes| next_offset(axes, handle, delta))
                        };
                        let below_could_move = chain.iter().any(|link| could_move(link).is_some());
                        let target = target_or(cx, root);
                        scrolled.pointer_input("wheel", target, &event.into(), window, cx);
                        let mut step_taken = window.default_prevented();
                        if !step_taken
                            && let Some(((_, handle), next)) = chain
                                .iter()
                                .find_map(|link| could_move(link).map(|next| (link, next)))
                        {
                            handle.set_offset(next);
                            window.refresh();
                            step_taken = true;
                        }
                        // The root's own listener takes the step when it is under the pointer.
                        cx.default_global::<RootStep>().0 =
                            (!step_taken && !below_could_move).then_some(delta);
                        chain.clear();
                        cx.default_global::<WheelChain>().0 = chain;
                    }
                },
            );
            let dispatch = dispatch.clone();
            window.on_mouse_event(move |_: &MouseExitEvent, phase, _, _| {
                if phase == DispatchPhase::Capture {
                    dispatch.pointer_left();
                }
            });
        },
    )
    .absolute()
    .size_0()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::element::pointer::tests::{LEAF, at, report};
    use crate::tests::*;
    use crate::tree::VirtualTree;
    use crate::{clear_presses, pressed_target};
    use gpui::prelude::*;
    use gpui::{
        MouseButton, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point, px,
    };
    use std::rc::Rc;

    /// Runs one of each mouse event at `pos`.
    fn mouse_events(cx: &mut VisualTestContext, pos: (f32, f32)) {
        cx.simulate_mouse_move(at(pos), None, gpui::Modifiers::none());
        cx.simulate_event(ScrollWheelEvent {
            position: at(pos),
            delta: ScrollDelta::Pixels(point(px(0.), px(-5.))),
            ..Default::default()
        });
        cx.simulate_click(at(pos), gpui::Modifiers::none());
    }

    /// `(event, target)` of each report, in order.
    fn reports(calls: &Calls) -> Vec<(String, NodeId)> {
        calls.borrow().iter().map(|c| (c.0.clone(), c.1)).collect()
    }

    /// Every `(event, target, pressed)` after the last `mousedown`.
    fn since_press(calls: &Calls) -> Vec<(String, NodeId, Option<NodeId>)> {
        let calls = calls.borrow();
        let last = calls.iter().rposition(|c| c.0 == "mousedown").unwrap();
        calls[last..]
            .iter()
            .map(|c| (c.0.clone(), c.1, c.3))
            .collect()
    }

    const MID: (f32, f32) = (80., 80.);

    const ROOT_ONLY: (f32, f32) = (150., 150.);

    const OUTSIDE: (f32, f32) = (300., 300.);

    #[gpui::test]
    fn every_input_is_reported_once_with_the_deepest_container_as_target(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        mouse_events(&mut scene.cx, LEAF);
        assert_eq!(
            reports(&scene.calls),
            [
                report("mousemove", 2),
                report("wheel", 2),
                report("mousedown", 2),
                report("mouseup", 2),
                report("release", 2),
            ]
        );
        assert!(
            scene.calls.borrow()[..4].iter().all(|c| c.2 == Some(2)),
            "every container below the root reported before the sink"
        );
    }

    #[gpui::test]
    fn the_target_is_cleared_for_each_event(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        mouse_events(&mut scene.cx, LEAF);
        scene.calls.borrow_mut().clear();
        mouse_events(&mut scene.cx, MID);
        let targets: Vec<_> = scene.calls.borrow().iter().map(|c| c.1).collect();
        assert_eq!(targets, [1; 5]);
    }

    #[gpui::test]
    fn the_root_is_the_target_when_only_the_root_is_under_the_pointer(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        mouse_events(&mut scene.cx, ROOT_ONLY);
        let calls = scene.calls.borrow();
        assert!(calls.iter().all(|c| c.1 == 0), "{calls:?}");
        assert!(
            calls[..4].iter().all(|c| c.2.is_none()),
            "the root marks itself after the tracker reports"
        );
    }

    #[gpui::test]
    fn a_sink_that_tracks_no_pointer_gets_the_root_as_every_target(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, false);
        mouse_events(&mut scene.cx, LEAF);
        let calls = scene.calls.borrow();
        assert_eq!(calls.len(), 5);
        assert!(
            calls[..4].iter().all(|c| c.1 == 0 && c.2.is_none()),
            "{calls:?}"
        );
        assert_eq!(calls[4].1, 0);
    }

    #[gpui::test]
    fn a_release_outside_every_container_reports_the_root(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Left, gpui::Modifiers::none());
        scene
            .cx
            .simulate_mouse_up(at(OUTSIDE), MouseButton::Left, gpui::Modifiers::none());
        assert_eq!(
            reports(&scene.calls)[1..],
            [report("mouseup", 0), report("release", 0)]
        );
    }

    #[gpui::test]
    fn the_pressed_target_is_the_press_container_while_the_release_runs(cx: &mut TestAppContext) {
        let none = gpui::Modifiers::none();
        let mut scene = probe_tree(cx, true);
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Left, none);
        scene.cx.simulate_mouse_up(at(MID), MouseButton::Left, none);
        let ev = |name: &str, target, pressed| (name.to_owned(), target, pressed);
        assert_eq!(
            since_press(&scene.calls)[1..],
            [ev("mouseup", 1, Some(2)), ev("release", 1, Some(2))]
        );
        assert_eq!(scene.cx.update(|_, cx| pressed_target(cx, 0)), None);
        scene
            .cx
            .simulate_mouse_down(at(MID), MouseButton::Left, none);
        let down = scene
            .calls
            .borrow()
            .iter()
            .rfind(|c| c.0 == "mousedown")
            .cloned();
        assert_eq!(down.map(|c| c.3), Some(Some(1)));
    }

    #[gpui::test]
    fn a_mouseup_is_followed_by_one_release_report_naming_its_press(cx: &mut TestAppContext) {
        let none = gpui::Modifiers::none();
        let mut scene = probe_tree(cx, true);
        let ev = |name: &str, target, pressed| (name.to_owned(), target, pressed);

        // A release after a completed click has no press to report.
        scene.cx.simulate_click(at(LEAF), none);
        scene
            .cx
            .simulate_mouse_up(at(LEAF), MouseButton::Left, none);
        assert_eq!(scene.calls.borrow().last().unwrap().3, None);

        // A press by another button is not the release's press.
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Left, none);
        scene
            .cx
            .simulate_mouse_up(at(LEAF), MouseButton::Right, none);
        assert_eq!(
            since_press(&scene.calls)[1..],
            [ev("mouseup", 2, None), ev("release", 2, None)]
        );

        // Every release reports, whichever button.
        let releases = |calls: &Calls| calls.borrow().iter().filter(|c| c.0 == "release").count();
        let before = releases(&scene.calls);
        for button in [MouseButton::Right, MouseButton::Middle] {
            scene.cx.simulate_mouse_down(at(LEAF), button, none);
            scene.cx.simulate_mouse_up(at(LEAF), button, none);
        }
        assert_eq!(releases(&scene.calls) - before, 2);
    }

    #[gpui::test]
    fn each_button_keeps_its_own_press_until_its_release(cx: &mut TestAppContext) {
        let none = gpui::Modifiers::none();
        let mut scene = probe_tree(cx, true);
        let release = |calls: &Calls| {
            let calls = calls.borrow();
            let last = calls.iter().rfind(|c| c.0 == "release").unwrap();
            (last.1, last.3)
        };

        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Left, none);
        scene
            .cx
            .simulate_mouse_down(at(MID), MouseButton::Right, none);
        scene
            .cx
            .simulate_mouse_up(at(LEAF), MouseButton::Left, none);
        assert_eq!(release(&scene.calls), (2, Some(2)));
        scene
            .cx
            .simulate_mouse_up(at(LEAF), MouseButton::Right, none);
        assert_eq!(release(&scene.calls), (2, Some(1)));
    }

    #[gpui::test]
    fn after_clearing_the_presses_a_release_reports_no_press(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        let none = gpui::Modifiers::none();

        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Left, none);
        scene.cx.update(|_, cx| clear_presses(cx));
        scene
            .cx
            .simulate_mouse_up(at(LEAF), MouseButton::Left, none);

        assert_eq!(since_press(&scene.calls).last().unwrap().2, None);
    }

    #[gpui::test]
    fn only_a_press_prevents_the_default_before_it_is_reported(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        mouse_events(&mut scene.cx, LEAF);
        let prevented: Vec<_> = scene.prevented.borrow().clone();
        let want = |event: &str, prevented| (event.to_owned(), prevented);
        assert_eq!(
            prevented,
            [
                want("mousemove", false),
                want("wheel", false),
                want("mousedown", true),
                want("mouseup", false),
            ]
        );
    }

    #[gpui::test]
    fn a_right_press_fires_contextmenu(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Right, gpui::Modifiers::none());
        assert_eq!(
            reports(&scene.calls),
            [report("mousedown", 2), report("fire contextmenu", 2)]
        );
    }

    #[gpui::test]
    fn a_middle_press_fires_no_contextmenu(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Middle, gpui::Modifiers::none());
        assert_eq!(reports(&scene.calls), [report("mousedown", 2)]);
    }

    /// An inert child never becomes the target, so the listening parent does.
    #[gpui::test]
    fn an_inert_child_is_never_the_event_target(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let child = tree.create_node("div").unwrap();
        for (id, side) in [(parent, 200.0), (child, 100.0)] {
            tree.set_style(id, "width", side).unwrap();
            tree.set_style(id, "height", side).unwrap();
        }
        tree.set_attribute(child, "inert", true).unwrap();
        tree.append_child(parent, child).unwrap();
        let probe = probe_of(tree);
        let calls = Rc::clone(&probe.calls);
        let window = cx.add_window(|_, _| ProbeView(probe));
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.simulate_mouse_move(at(LEAF), None, gpui::Modifiers::none());

        assert_eq!(reports(&calls), [report("mousemove", parent)]);
    }

    #[gpui::test]
    fn tracking_the_pointer_leaves_layout_alone(cx: &mut TestAppContext) {
        let bounds = |cx: &mut VisualTestContext| -> Vec<_> {
            ["node-0", "node-1", "node-2"]
                .map(|selector| cx.debug_bounds(selector).unwrap())
                .to_vec()
        };
        let mut tracked = probe_tree(cx, true);
        let tracked = bounds(&mut tracked.cx);
        let mut plain = probe_tree(cx, false);
        assert_eq!(tracked, bounds(&mut plain.cx));
    }
}
