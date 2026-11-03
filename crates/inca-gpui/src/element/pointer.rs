// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::prelude::*;
use gpui::{
    App, DispatchPhase, Global, Modifiers, MouseButton, MouseDownEvent, MouseExitEvent,
    MouseMoveEvent, MouseUpEvent, ScrollHandle, ScrollWheelEvent, Window, canvas,
};

use super::scroll::{RootStep, WheelChain, next_offset};
use crate::event_sink::{EventPayload, EventSink, button_index};
use crate::tree::NodeId;

mod tracker;

pub(super) use tracker::pointer_tracker;

// The first container to report in the bubble phase wins; a release clears its button's press.
#[derive(Default)]
struct MouseTarget {
    current: Option<NodeId>,
    /// The press container and click count of each button, indexed by
    /// `MousePayload::button`.
    pressed: [Option<(NodeId, usize)>; 5],
}

impl Global for MouseTarget {}

const MACOS: bool = cfg!(target_os = "macos");

/// Whether Control is held, and whether the platform reports a Control+click
/// as a right press.
#[derive(Clone, Copy)]
struct ControlClick {
    held: bool,
    macos: bool,
}

impl Default for ControlClick {
    fn default() -> Self {
        Self {
            held: false,
            macos: MACOS,
        }
    }
}

impl Global for ControlClick {}

fn control_click(cx: &App) -> ControlClick {
    cx.try_global::<ControlClick>().copied().unwrap_or_default()
}

/// The button and modifiers of a press the platform reported as a right press
/// without Control while Control is held: a left press with Control.
fn restore_control_click(
    button: MouseButton,
    modifiers: Modifiers,
    click: ControlClick,
) -> (MouseButton, Modifiers) {
    if click.macos && button == MouseButton::Right && !modifiers.control && click.held {
        return (
            MouseButton::Left,
            Modifiers {
                control: true,
                ..modifiers
            },
        );
    }
    (button, modifiers)
}

/// Whether a press of `button` with `modifiers` opens the context menu.
#[must_use]
pub fn opens_context_menu(button: MouseButton, modifiers: Modifiers, macos: bool) -> bool {
    button == MouseButton::Right || (macos && button == MouseButton::Left && modifiers.control)
}

/// Whether a release completes a `click`: on macOS a left release with
/// Control held completes none.
fn completes_click(button: MouseButton, modifiers: Modifiers, macos: bool) -> bool {
    !(macos && button == MouseButton::Left && modifiers.control)
}

fn restored_down(event: &MouseDownEvent, cx: &App) -> MouseDownEvent {
    let (button, modifiers) =
        restore_control_click(event.button, event.modifiers, control_click(cx));
    MouseDownEvent {
        button,
        modifiers,
        ..event.clone()
    }
}

fn restored_up(event: &MouseUpEvent, cx: &App) -> MouseUpEvent {
    let (button, modifiers) =
        restore_control_click(event.button, event.modifiers, control_click(cx));
    MouseUpEvent {
        button,
        modifiers,
        ..event.clone()
    }
}

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
            let event = restored_down(event, cx);
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
        cx.default_global::<ControlClick>().held = event.modifiers.control;
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
    use crate::tests::*;
    use gpui::{MouseButton, Pixels, Point, TestAppContext, point, px};
    use inca_test_macros::gpui_cases;
    use rstest::rstest;

    pub(super) fn at((x, y): (f32, f32)) -> Point<Pixels> {
        point(px(x), px(y))
    }

    pub(super) fn report(event: &str, target: NodeId) -> (String, NodeId) {
        (event.to_owned(), target)
    }

    pub(super) const LEAF: (f32, f32) = (10., 10.);

    /// Sets the platform and focuses the root, as the host does, so the window's modifier events
    /// reach the root.
    fn control_click_of(scene: &mut Scene, macos: bool) {
        let handle = scene.cx.update(|_, cx| {
            cx.set_global(ControlClick { held: false, macos });
            cx.focus_handle()
        });
        *scene.focus.borrow_mut() = Some((0, handle.clone()));
        scene.cx.update(|window, cx| {
            window.refresh();
            window.focus(&handle, cx);
        });
    }

    /// `(button, control)` of the last payload reported for `event`.
    fn last_payload(scene: &Scene, event: &str) -> (u8, bool) {
        let payloads = scene.payloads.borrow();
        match payloads.iter().rfind(|p| p.0 == event).map(|p| &p.1) {
            Some(EventPayload::Mouse(mouse)) => (mouse.button, mouse.modifiers.control),
            other => panic!("no mouse payload for {event}: {other:?}"),
        }
    }

    /// The press the last `release` report names.
    fn released_press(scene: &Scene) -> Option<NodeId> {
        let calls = scene.calls.borrow();
        calls.iter().rfind(|c| c.0 == "release").unwrap().3
    }

    const CONTROL: Modifiers = Modifiers {
        control: true,
        alt: false,
        shift: false,
        platform: false,
        function: false,
    };

    #[rstest]
    #[case::macos_rewritten(
        true,
        true,
        MouseButton::Right,
        Modifiers::none(),
        MouseButton::Left,
        true
    )]
    #[case::macos_real_right(
        true,
        false,
        MouseButton::Right,
        Modifiers::none(),
        MouseButton::Right,
        false
    )]
    #[case::macos_control_right(true, true, MouseButton::Right, CONTROL, MouseButton::Right, true)]
    #[case::linux(
        false,
        true,
        MouseButton::Right,
        Modifiers::none(),
        MouseButton::Right,
        false
    )]
    #[case::left(
        true,
        true,
        MouseButton::Left,
        Modifiers::none(),
        MouseButton::Left,
        false
    )]
    fn restore_control_click_turns_a_rewritten_press_left(
        #[case] macos: bool,
        #[case] held: bool,
        #[case] button: MouseButton,
        #[case] modifiers: Modifiers,
        #[case] want_button: MouseButton,
        #[case] want_control: bool,
    ) {
        let restored = restore_control_click(button, modifiers, ControlClick { held, macos });
        assert_eq!(
            restored,
            (
                want_button,
                Modifiers {
                    control: want_control,
                    ..modifiers
                }
            )
        );
    }

    #[rstest]
    #[case::right(MouseButton::Right, Modifiers::none(), false, true)]
    #[case::mac_control_left(MouseButton::Left, CONTROL, true, true)]
    #[case::linux_control_left(MouseButton::Left, CONTROL, false, false)]
    #[case::mac_plain_left(MouseButton::Left, Modifiers::none(), true, false)]
    fn opens_context_menu_follows_the_button_and_platform(
        #[case] button: MouseButton,
        #[case] modifiers: Modifiers,
        #[case] macos: bool,
        #[case] want: bool,
    ) {
        assert_eq!(opens_context_menu(button, modifiers, macos), want);
    }

    #[rstest]
    #[case::macos_control_left(MouseButton::Left, CONTROL, true, false)]
    #[case::macos_plain_left(MouseButton::Left, Modifiers::none(), true, true)]
    #[case::linux_control_left(MouseButton::Left, CONTROL, false, true)]
    #[case::macos_control_right(MouseButton::Right, CONTROL, true, true)]
    fn completes_click_follows_the_platform(
        #[case] button: MouseButton,
        #[case] modifiers: Modifiers,
        #[case] macos: bool,
        #[case] want: bool,
    ) {
        assert_eq!(completes_click(button, modifiers, macos), want);
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::macos(true, (0, true), 0)]
    #[case::linux(false, (2, false), 2)]
    fn a_rewritten_control_click_reports_a_left_press_with_control(
        cx: &mut TestAppContext,
        macos: bool,
        down: (u8, bool),
        contextmenu: u8,
    ) {
        let mut scene = probe_tree(cx, true);
        control_click_of(&mut scene, macos);
        scene.cx.simulate_modifiers_change(CONTROL);
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Right, Modifiers::none());
        assert_eq!(last_payload(&scene, "mousedown"), down);
        assert_eq!(last_payload(&scene, "contextmenu").0, contextmenu);
    }

    #[gpui::test]
    fn a_pointer_move_with_control_counts_as_held(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        control_click_of(&mut scene, true);
        scene.cx.simulate_mouse_move(at(LEAF), None, CONTROL);
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Right, Modifiers::none());
        assert_eq!(last_payload(&scene, "mousedown"), (0, true));
    }

    #[gpui::test]
    fn a_control_release_clears_the_held_control(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        control_click_of(&mut scene, true);
        scene.cx.simulate_modifiers_change(CONTROL);
        scene.cx.simulate_modifiers_change(Modifiers::none());
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Right, Modifiers::none());
        assert_eq!(last_payload(&scene, "mousedown"), (2, false));
    }

    #[gpui::test]
    fn a_rewritten_release_is_a_left_release(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        control_click_of(&mut scene, true);
        scene.cx.simulate_modifiers_change(CONTROL);
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Right, Modifiers::none());
        scene
            .cx
            .simulate_mouse_up(at(LEAF), MouseButton::Right, Modifiers::none());
        assert_eq!(last_payload(&scene, "mouseup"), (0, true));
        assert_eq!(released_press(&scene), None);
    }

    #[gpui::test]
    fn a_release_after_control_completes_the_click(cx: &mut TestAppContext) {
        let mut scene = probe_tree(cx, true);
        control_click_of(&mut scene, true);
        scene.cx.simulate_modifiers_change(CONTROL);
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Right, Modifiers::none());
        scene.cx.simulate_modifiers_change(Modifiers::none());
        scene
            .cx
            .simulate_mouse_up(at(LEAF), MouseButton::Left, Modifiers::none());
        assert_eq!(released_press(&scene), Some(2));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::macos(true, None)]
    #[case::linux(false, Some(2))]
    fn a_release_with_control_held_completes_no_click_on_macos(
        cx: &mut TestAppContext,
        macos: bool,
        pressed: Option<NodeId>,
    ) {
        let mut scene = probe_tree(cx, true);
        control_click_of(&mut scene, macos);
        scene.cx.simulate_modifiers_change(CONTROL);
        scene
            .cx
            .simulate_mouse_down(at(LEAF), MouseButton::Left, CONTROL);
        scene
            .cx
            .simulate_mouse_up(at(LEAF), MouseButton::Left, CONTROL);
        assert_eq!(released_press(&scene), pressed);
    }
}
