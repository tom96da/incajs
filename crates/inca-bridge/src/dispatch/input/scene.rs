// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The test scene of the input dispatch: an `input` and a `button` in an active window, and the
//! helpers that log and compare the events they receive.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, AppContext as _, Keystroke, TestAppContext, WindowHandle};
use inca_gpui::{EventPayload, EventSink, InputRequest, InputState, MousePayload, NodeId};
use serde_json::{Value, json};

use crate::test_support::{TestRoot, engine_for, mount_active};
use crate::tests::{at, listen};
use crate::{EventDispatcher, Host};

/// An `input` holding a value (100 px wide, 6 px per char, from x 0) and a `button`, under a `div`
/// in an active window.
pub(crate) struct InputScene {
    pub(crate) window: WindowHandle<TestRoot>,
    pub(crate) dispatcher: EventDispatcher,
    pub(crate) host: Rc<RefCell<Host>>,
    pub(crate) engine: Rc<inca_jsenv::Engine>,
    pub(crate) parent: NodeId,
    pub(crate) input: NodeId,
    pub(crate) button: NodeId,
}

pub(crate) fn input_scene(cx: &mut TestAppContext, value: &str) -> InputScene {
    let host = Rc::new(RefCell::new(Host::default()));
    let (parent, input, button) = {
        let mut h = host.borrow_mut();
        let root = h.root();
        let [parent, input, button] =
            ["div", "input", "button"].map(|tag| h.tree.create_node(tag).unwrap());
        h.tree.append_child(root, parent).unwrap();
        h.tree.append_child(parent, input).unwrap();
        h.tree.append_child(parent, button).unwrap();
        h.tree.set_style(input, "width", 100.0).unwrap();
        h.tree.set_style(input, "text_size", 10.0).unwrap();
        h.focus.mark_tab_dirty(input);
        h.focus.mark_tab_dirty(button);
        h.inputs
            .get_mut()
            .entry(input)
            .or_default()
            .set_value(value);
        (parent, input, button)
    };
    let engine = engine_for(&host);
    engine.eval::<()>("globalThis.out = [];").unwrap();
    let window = mount_active(cx, &host, &engine, parent);
    let dispatcher = window
        .read_with(cx, |root, _| root.dispatcher.clone())
        .unwrap();
    InputScene {
        window,
        dispatcher,
        host,
        engine,
        parent,
        input,
        button,
    }
}

impl InputScene {
    pub(crate) fn in_window<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&Self, &mut gpui::Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.window.into(), |_, window, cx| f(self, window, cx))
            .unwrap()
    }

    pub(crate) fn draw(&self, cx: &mut TestAppContext) {
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
    }

    pub(crate) fn focus(&self, cx: &mut TestAppContext, node: NodeId) {
        self.host.borrow_mut().focus.request_focus(node, false);
        self.draw(cx);
        cx.run_until_parked();
    }

    /// Dispatches the space-separated keystrokes and returns whether the last one was handled.
    pub(crate) fn key(&self, cx: &mut TestAppContext, keys: &str) -> bool {
        let mut handled = false;
        for key in keys.split(' ') {
            let keystroke = Keystroke::parse(key).unwrap();
            handled = self.in_window(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx));
        }
        cx.run_until_parked();
        self.draw(cx);
        handled
    }

    pub(crate) fn request(&self, cx: &mut TestAppContext, request: InputRequest) {
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .input_request(scene.input, request, window, cx);
        });
    }

    pub(crate) fn deactivate(&self, cx: &mut TestAppContext) {
        self.in_window(cx, |scene, window, cx| {
            scene.dispatcher.window_activated(false, window, cx);
        });
    }

    /// Sends the raw pointer `event` to `node`.
    pub(crate) fn pointer(
        &self,
        cx: &mut TestAppContext,
        event: &str,
        node: NodeId,
        mouse: MousePayload,
    ) {
        let payload = EventPayload::Mouse(mouse);
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .pointer_input(event, node, &payload, window, cx);
        });
    }

    pub(crate) fn press(&self, cx: &mut TestAppContext, x: f32, shift: bool) {
        self.pointer(cx, "mousedown", self.input, mouse_of(x, 0, shift));
    }

    pub(crate) fn release(&self, cx: &mut TestAppContext, x: f32) {
        self.pointer(cx, "mouseup", self.input, mouse_of(x, 0, false));
    }

    pub(crate) fn state(&self) -> InputState {
        self.host.borrow().inputs.borrow()[&self.input].clone()
    }

    /// Selects `anchor` to `focus` in the input.
    pub(crate) fn select(&self, anchor: usize, focus: usize) {
        let host = self.host.borrow();
        let mut inputs = host.inputs.borrow_mut();
        inputs.get_mut(&self.input).unwrap().select(anchor, focus);
    }

    pub(crate) fn set_attribute(&self, name: &str) {
        let mut host = self.host.borrow_mut();
        host.tree.set_attribute(self.input, name, true).unwrap();
    }
}

/// Logs `[type, inputType, data, isComposing, cancelable, value]` of each event at the input,
/// the fields an event has not read as `null`.
const LOG: &str = r"
    globalThis.log = [];
    globalThis.rec = (e) => {
        const edit = e.type === 'beforeinput' || e.type === 'input';
        const composition = e.type.startsWith('composition');
        const key = e.type.startsWith('key');
        globalThis.log.push([
            e.type,
            edit ? e.inputType : null,
            edit || composition ? e.data : null,
            edit || key ? e.isComposing : null,
            e.cancelable,
            __inca_native__.inputValue(INPUT),
        ]);
    };
    ";

/// Starts the log and records `events` at `node`.
pub(crate) fn record(s: &InputScene, node: NodeId, events: &[&str]) {
    s.engine
        .eval::<()>(&LOG.replace("INPUT", &s.input.to_string()))
        .unwrap();
    for event in events {
        listen(&s.dispatcher, node, event, "rec");
    }
}

/// Takes the rows logged so far.
pub(crate) fn log(s: &InputScene) -> Vec<Value> {
    let json = s
        .engine
        .eval::<String>("JSON.stringify(globalThis.log.splice(0))")
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

/// Takes the types of the rows logged so far.
pub(crate) fn types(s: &InputScene) -> Vec<String> {
    log(s)
        .iter()
        .map(|row| row[0].as_str().unwrap().to_owned())
        .collect()
}

/// A mouse payload of `button` at window x `x` and y 5.
pub(crate) fn mouse_of(x: f32, button: u8, shift: bool) -> MousePayload {
    let modifiers = gpui::Modifiers {
        shift,
        ..gpui::Modifiers::none()
    };
    MousePayload {
        button,
        ..MousePayload::at(at(x, 5.0), modifiers)
    }
}

pub(crate) fn replace(text: &str) -> InputRequest {
    InputRequest::Replace {
        range: None,
        text: text.to_owned(),
    }
}

pub(crate) fn mark(text: &str) -> InputRequest {
    InputRequest::Mark {
        range: None,
        text: text.to_owned(),
        selected: None,
    }
}

/// Sets each of `names` on the input of `s`.
pub(crate) fn set_attributes(s: &InputScene, names: &[&str]) {
    for name in names {
        s.set_attribute(name);
    }
}

pub(crate) fn row(
    kind: &str,
    input_type: Option<&str>,
    data: Option<&str>,
    composing: Option<bool>,
    cancelable: bool,
    value: &str,
) -> Value {
    json!([kind, input_type, data, composing, cancelable, value])
}

/// The `keydown` row of a key typed at an input holding `value`.
pub(crate) fn keydown(composing: bool, value: &str) -> Value {
    row("keydown", None, None, Some(composing), true, value)
}

/// Runs `js` with `INPUT` replaced by the id of the input.
pub(crate) fn eval_input(s: &InputScene, js: &str) {
    s.engine
        .eval::<()>(&js.replace("INPUT", &s.input.to_string()))
        .unwrap();
}

pub(crate) fn cancel_beforeinput(s: &InputScene) {
    listen(
        &s.dispatcher,
        s.input,
        "beforeinput",
        "(e) => e.preventDefault()",
    );
}

/// The rows of an `Enter` at an input holding "x": `beforeinput` for a line break, then
/// `change`.
pub(crate) fn line_break_rows() -> Vec<Value> {
    vec![
        row(
            "beforeinput",
            Some("insertLineBreak"),
            None,
            Some(false),
            true,
            "x",
        ),
        row("change", None, None, None, false, "x"),
    ]
}
