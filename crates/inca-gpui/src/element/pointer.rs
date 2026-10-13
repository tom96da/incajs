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
                        pressed.pointer_pressed();
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
