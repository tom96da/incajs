// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The sink that receives the events a dispatch fires.

use super::{
    App, EventPayload, InputLayout, InputRequest, InputState, MouseUpEvent, NodeId, NodeLayout,
    Window,
};

/// What [`crate::element`] needs from something that can dispatch a native
/// event into JS. The trait keeps this crate free of a `QuickJS` dependency.
/// An implementor provides every method.
pub trait EventSink {
    /// Whether the containers report the pointer. The render path asks once
    /// per frame. With `true`, every container that is not inert becomes a
    /// pointer target, which gives it a hitbox and a hover state.
    fn tracks_pointer(&self) -> bool;

    /// Fires `event` at `start`, with `start` as its target. The event runs a
    /// capture pass over the ancestors of `start`, then a bubble pass that
    /// reaches the ancestors when its kind bubbles. `preventDefault()` calls
    /// from passive listeners are ignored. Returns whether a listener
    /// prevented the default action.
    fn fire(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) -> bool;

    /// The handle to track this node's focus with, if it's focusable.
    fn focus_handle(&self, node_id: NodeId) -> Option<gpui::FocusHandle>;

    /// Tracks this node's scroll offset. A scroll container (`hidden` or
    /// `scroll` on either axis) gets one. After the `wheel` listeners ran, the
    /// wheel step goes to the innermost container under the pointer that can
    /// move, and is not applied after `preventDefault()`.
    fn scroll_handle(&self, node_id: NodeId) -> Option<gpui::ScrollHandle>;

    /// The axes along which a wheel step can scroll `node_id` now, read from
    /// the live tree (see [`crate::wheel_axes`]). `None` when the node is unknown,
    /// detached, hidden or inert. A shown container without `scroll` overflow
    /// gives `false` on both axes.
    fn wheel_axes(&self, node_id: NodeId) -> Option<crate::WheelAxes>;

    /// Forgets every recorded layout. Called before each frame's tree is
    /// built, so only nodes laid out in the new frame have a record.
    fn clear_layout(&self);

    /// Records the box `node_id` was laid out in. Called during prepaint for
    /// each displayed child of a container, as the frame is drawn.
    fn record_layout(&self, node_id: NodeId, layout: NodeLayout);

    /// Reports a pointer move to `position`, in window coordinates. Called
    /// for every pointer move in the window, buttons held included, before
    /// any container reports. `cx` lets the sink schedule work for the end
    /// of the current update.
    fn pointer_moved(&self, position: gpui::Point<gpui::Pixels>, cx: &mut App);

    /// Reports that the pointer left the window. The next
    /// [`EventSink::pointer_moved`] starts a new measurement.
    fn pointer_left(&self);

    /// Reports the window's modifier keys after a change. The sink fires
    /// `keydown` for each modifier pressed and `keyup` for each released, at
    /// the focused node or the root.
    fn modifiers_changed(&self, modifiers: gpui::Modifiers, window: &mut Window, cx: &mut App);

    /// Reports that `node_id` became hovered or stopped being hovered, as
    /// `gpui` decides it. The sink fires the pointer and mouse `over`, `out`,
    /// `enter` and `leave` events for the net change once the current update
    /// ends.
    fn hover_changed(&self, node_id: NodeId, hovered: bool, window: &mut Window, cx: &mut App);

    /// Reports `node_id` as the deepest container under the pointer for the
    /// pointer move being handled, before the move itself is reported. The
    /// sink fires the hover events for it at once.
    fn pointer_over(&self, node_id: NodeId, window: &mut Window, cx: &mut App);

    /// Reports one `mousedown`, `mouseup`, `mousemove` or `wheel` (`event`)
    /// once every container has reported itself. `target` is the deepest
    /// container under the pointer, or the root when none is. The sink
    /// dispatches the event along the path of `target` and runs its default
    /// action. A `mousedown`, `mouseup` or `mousemove` is preceded by the
    /// pointer event of the input. Both go to the node holding a pointer
    /// capture, else to `target`. A `mousedown` focuses the nearest
    /// focusable node on the path and leaves the focus alone when a listener
    /// prevented the default or the `pointerdown` was canceled.
    fn pointer_input(
        &self,
        event: &str,
        target: NodeId,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    );

    /// Reports a mouse button release right after its `mouseup`.
    /// `event.click_count` is the click count of the press of the same
    /// button, and the release's own count when `pressed` is `None`.
    /// `pressed` is the deepest container under the press of the same
    /// button, and `None` when no such press is recorded. `released` is the
    /// deepest container under the release, or the root when none is. The
    /// sink fires `click` for the left button, `auxclick` for the others and
    /// `dblclick` after the second `click`, at the nearest common ancestor of
    /// the two containers. With `pressed` `None`, nothing fires.
    fn pointer_released(
        &self,
        event: &MouseUpEvent,
        pressed: Option<NodeId>,
        released: NodeId,
        window: &mut Window,
        cx: &mut App,
    );

    /// Reports a key press (`up` false) or release before any other key
    /// listener of the window runs. The sink dispatches `keydown` or `keyup`,
    /// and `keypress` after an uncanceled `keydown` of a key that types a
    /// character, to the focused node, or to the root when none is focused,
    /// then runs the default action unless a listener prevented it. Tab and
    /// Shift+Tab move focus; Control, Alt or Meta cancel the move. On a
    /// focused enabled button, Enter clicks it on press and Space clicks it
    /// on release. A pending Space press ends at an Enter click, a Space key
    /// down, a Space key up or when the button loses focus. While a mouse
    /// press on the focused button is held, Space up also clicks it, once. A
    /// click, the mouse release or the button losing focus ends the hold.
    /// When a listener prevented the default, the sink calls
    /// `Window::prevent_default` and `App::stop_propagation`. At a focused
    /// `input` the default action edits it, and an edit key counts as
    /// handled.
    fn key_input(
        &self,
        keystroke: &gpui::Keystroke,
        up: bool,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    );

    /// The text state of the `input` node `node_id`, `None` for any other
    /// node.
    fn input(&self, node_id: NodeId) -> Option<InputState>;

    /// Stores where the frame being drawn put the text of `node_id`.
    fn record_input(&self, node_id: NodeId, layout: InputLayout);

    /// Carries a call of the platform's text input for the focused `input`
    /// `node_id`: an insertion, a composition, the end of a composition or a
    /// selection. A `disabled` input applies a selection only. A `readonly` one
    /// also reports typed text as `beforeinput` while no composition runs.
    fn input_request(
        &self,
        node_id: NodeId,
        request: InputRequest,
        window: &mut Window,
        cx: &mut App,
    );
}

/// Emits a body for each listed [`EventSink`] method, with the trait's
/// signature: empty, or `None` for `input`. A method missing from the trait
/// is a compile error.
macro_rules! noop_sink_methods {
    ($($name:ident),* $(,)?) => { $(noop_sink_methods!(@one $name);)* };
    (@one clear_layout) => { fn clear_layout(&self) {} };
    (@one record_layout) => {
        fn record_layout(&self, _: $crate::tree::NodeId, _: $crate::element::NodeLayout) {}
    };
    (@one pointer_moved) => {
        fn pointer_moved(&self, _: gpui::Point<gpui::Pixels>, _: &mut gpui::App) {}
    };
    (@one pointer_left) => { fn pointer_left(&self) {} };
    (@one modifiers_changed) => {
        fn modifiers_changed(&self, _: gpui::Modifiers, _: &mut gpui::Window, _: &mut gpui::App) {}
    };
    (@one hover_changed) => {
        fn hover_changed(
            &self,
            _: $crate::tree::NodeId,
            _: bool,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    };
    (@one pointer_over) => {
        fn pointer_over(&self, _: $crate::tree::NodeId, _: &mut gpui::Window, _: &mut gpui::App) {}
    };
    (@one pointer_input) => {
        fn pointer_input(
            &self,
            _: &str,
            _: $crate::tree::NodeId,
            _: &$crate::event_sink::EventPayload,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    };
    (@one pointer_released) => {
        fn pointer_released(
            &self,
            _: &gpui::MouseUpEvent,
            _: Option<$crate::tree::NodeId>,
            _: $crate::tree::NodeId,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    };
    (@one key_input) => {
        fn key_input(
            &self,
            _: &gpui::Keystroke,
            _: bool,
            _: &$crate::event_sink::EventPayload,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    };
    (@one input) => {
        fn input(&self, _: $crate::tree::NodeId) -> Option<$crate::input::InputState> {
            None
        }
    };
    (@one record_input) => {
        fn record_input(&self, _: $crate::tree::NodeId, _: $crate::input::InputLayout) {}
    };
    (@one input_request) => {
        fn input_request(
            &self,
            _: $crate::tree::NodeId,
            _: $crate::input::InputRequest,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    };
}
pub(crate) use noop_sink_methods;
