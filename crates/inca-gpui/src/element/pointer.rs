// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::prelude::*;
use gpui::{
    App, DispatchPhase, Global, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent,
    MouseUpEvent, ScrollWheelEvent, Window, canvas,
};

use super::scroll::roll_back_extra_steps;
use crate::event_sink::{EventPayload, EventSink, button_index};
use crate::tree::NodeId;

// The first container to report in the bubble phase wins; a release clears its button's press.
#[derive(Default)]
struct MouseTarget {
    current: Option<NodeId>,
    /// The press container and click count of each button, indexed by
    /// `MousePayload::button`.
    pressed: [Option<(NodeId, usize)>; 5],
}

impl Global for MouseTarget {}

/// The deepest container under the pointer for the mouse event being
/// handled, and after it until the next mouse event starts. `None` when no
/// container has reported, and for a sink that tracks no pointer.
#[must_use]
pub fn mouse_target(cx: &App) -> Option<NodeId> {
    cx.try_global::<MouseTarget>().and_then(|t| t.current)
}

/// The deepest container under the pointer at the last press of `button`
/// (`MousePayload::button`), until a release of that button has been
/// reported.
#[must_use]
pub fn pressed_target(cx: &App, button: u8) -> Option<NodeId> {
    cx.try_global::<MouseTarget>()
        .and_then(|t| t.pressed.get(usize::from(button)).copied().flatten())
        .map(|(id, _)| id)
}

/// Forgets every recorded press. A release that follows reports no press.
pub fn clear_presses(cx: &mut App) {
    cx.default_global::<MouseTarget>().pressed = Default::default();
}

/// The index of `button` in [`MouseTarget::pressed`].
fn slot(button: MouseButton) -> usize {
    usize::from(button_index(button))
}

fn mark_target(cx: &mut App, id: NodeId) {
    let target = cx.default_global::<MouseTarget>();
    target.current.get_or_insert(id);
}

/// Reports `id` as the event's target when no deeper container has.
pub(super) fn mark_mouse_target<Elem: InteractiveElement + FluentBuilder>(
    element: Elem,
    id: NodeId,
    sink: Option<impl EventSink + 'static>,
) -> Elem {
    let mut element = element
        .on_any_mouse_down(move |event, _, cx| {
            mark_target(cx, id);
            let slot = slot(event.button);
            cx.default_global::<MouseTarget>().pressed[slot].get_or_insert((id, event.click_count));
        })
        .on_mouse_move(move |_, window, cx| {
            // The first container to report is the deepest one.
            let deepest = mouse_target(cx).is_none();
            mark_target(cx, id);
            if deepest && let Some(sink) = &sink {
                sink.pointer_over(id, window, cx);
            }
        })
        .on_scroll_wheel(move |_, _, cx| mark_target(cx, id));
    element
        .interactivity()
        .on_any_mouse_up(move |_, _, cx| mark_target(cx, id));
    element
}

/// Reports `id`'s hover state changes to `dispatch`. `gpui` panics when a
/// node calls `on_hover` twice, so this is the only caller.
pub(super) fn wire_hover<Elem, E>(element: Elem, dispatch: Option<&E>, id: NodeId) -> Elem
where
    Elem: StatefulInteractiveElement + FluentBuilder,
    E: EventSink + Clone + 'static,
{
    element.when_some(dispatch.cloned(), |el, sink| {
        // Hover state ignores the last input kind, so a key press keeps it.
        el.hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
            .on_hover(move |hovered, window, cx| sink.hover_changed(id, *hovered, window, cx))
    })
}

/// Reports each `ModifiersChangedEvent` of the window to the sink, which fires
/// the modifier key events. The root sees every one.
pub(super) fn wire_modifier_keys<Elem, E>(element: Elem, dispatch: &E) -> Elem
where
    Elem: InteractiveElement,
    E: EventSink + Clone + 'static,
{
    let sink = dispatch.clone();
    element.on_modifiers_changed(move |event, window, cx| {
        sink.modifiers_changed(event.modifiers, window, cx);
    })
}

/// Tracks `node_id`'s focus state on `element`, if it has a
/// [`gpui::FocusHandle`]. `.track_focus` is an [`InteractiveElement`] method,
/// so this applies the same way whether or not the container also got a
/// `gpui` `ElementId`.
pub(super) fn wire_focus<Elem, E>(element: Elem, dispatch: Option<&E>, id: NodeId) -> Elem
where
    Elem: InteractiveElement + FluentBuilder,
    E: EventSink,
{
    element.when_some(dispatch.and_then(|d| d.focus_handle(id)), |el, handle| {
        el.track_focus(&handle)
    })
}

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
    let event = with_press_count(event, cx);
    sink.pointer_input("mouseup", released, &(&event).into(), window, cx);
    sink.pointer_released(&event, pressed, released, window, cx);
    cx.default_global::<MouseTarget>().pressed[slot] = None;
}

// Zero-size first child of the root. Its capture handlers run before any
// container's, and its bubble handlers run after every container's below the
// root, so each input is reported once, with its target settled.
pub(super) fn pointer_tracker<E: EventSink + Clone + 'static>(
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
                        moved.pointer_moved(event.position, cx);
                    }
                    DispatchPhase::Bubble => {
                        let target = target_or(cx, root);
                        moved.pointer_input("mousemove", target, &event.into(), window, cx);
                    }
                },
            );
            let pressed = dispatch.clone();
            window.on_mouse_event(
                move |event: &MouseDownEvent, phase, window, cx| match phase {
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
                        if event.button == gpui::MouseButton::Right {
                            let payload = EventPayload::context_menu(event);
                            pressed.fire(target, "contextmenu", &payload, window, cx);
                        }
                    }
                },
            );
            let released = dispatch.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| match phase {
                DispatchPhase::Capture => cx.default_global::<MouseTarget>().current = None,
                DispatchPhase::Bubble => report_release(&released, root, event, window, cx),
            });
            let scrolled = dispatch.clone();
            window.on_mouse_event(
                move |event: &ScrollWheelEvent, phase, window, cx| match phase {
                    DispatchPhase::Capture => cx.default_global::<MouseTarget>().current = None,
                    DispatchPhase::Bubble => {
                        roll_back_extra_steps(cx);
                        let target = target_or(cx, root);
                        scrolled.pointer_input("wheel", target, &event.into(), window, cx);
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

// The root's capture handlers run before any other key listener of the
// window, so the sink sees every key once, whatever is focused.
pub(super) fn root_keys<Elem, E>(element: Elem, dispatch: E) -> Elem
where
    Elem: InteractiveElement,
    E: EventSink + Clone + 'static,
{
    let up = dispatch.clone();
    element
        .capture_key_down(move |event, window, cx| {
            dispatch.key_input(&event.keystroke, false, &event.into(), window, cx);
        })
        .capture_key_up(move |event, window, cx| {
            up.key_input(&event.keystroke, true, &event.into(), window, cx);
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::element::tests::*;
    use crate::tree::VirtualTree;
    use gpui::{Pixels, Point, px};
    use inca_test_macros::gpui_cases;
    use std::rc::Rc;

    use gpui::{
        MouseButton, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point,
    };

    const LEAF: (f32, f32) = (10., 10.);
    const MID: (f32, f32) = (80., 80.);
    const ROOT_ONLY: (f32, f32) = (150., 150.);
    const OUTSIDE: (f32, f32) = (300., 300.);

    fn at((x, y): (f32, f32)) -> Point<Pixels> {
        point(px(x), px(y))
    }

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

    fn report(event: &str, target: NodeId) -> (String, NodeId) {
        (event.to_owned(), target)
    }

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

    /// Every `(event, target, pressed)` after the last `mousedown`.
    fn since_press(calls: &Calls) -> Vec<(String, NodeId, Option<NodeId>)> {
        let calls = calls.borrow();
        let last = calls.iter().rposition(|c| c.0 == "mousedown").unwrap();
        calls[last..]
            .iter()
            .map(|c| (c.0.clone(), c.1, c.3))
            .collect()
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

    #[gpui_cases]
    #[gpui::test]
    #[case::right(MouseButton::Right, true)]
    #[case::middle(MouseButton::Middle, false)]
    fn a_press_fires_contextmenu_only_for_the_right_button(
        cx: &mut TestAppContext,
        button: MouseButton,
        contextmenu: bool,
    ) {
        let mut scene = probe_tree(cx, true);
        scene
            .cx
            .simulate_mouse_down(at(LEAF), button, gpui::Modifiers::none());
        let mut want = vec![report("mousedown", 2)];
        if contextmenu {
            want.push(report("fire contextmenu", 2));
        }
        assert_eq!(reports(&scene.calls), want);
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
        let probe = probe();
        let calls = Rc::clone(&probe.calls);
        let window = cx.add_window(|_, _| ProbeView(tree, probe));
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.simulate_mouse_move(at(LEAF), None, gpui::Modifiers::none());

        assert_eq!(reports(&calls), [report("mousemove", parent)]);
    }
}
