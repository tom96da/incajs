// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The input scene and the log of the events its editing fires.

#![allow(clippy::unwrap_used, dead_code, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Keystroke, MouseButton, MouseUpEvent, TestAppContext, VisualTestContext,
    Window, WindowHandle, WindowId,
};
use inca_document::{EVENTS, PointerSource, WheelPayload};
use inca_document::{EventPayload, EventTargetId, KeyPayload, MousePayload};
use inca_gpui::{EventSink, InputLayout, InputRequest};
use inca_node::NodeId;
use inca_style::set_style;
use inca_window::{Cells, EventDispatcher};

use super::rig::{self as wiring, *};

/// An `input` holding a value (100 px wide, 6 px per char, from x 0) and a `button`, under a `div`
/// in an active window, on an engine.
pub struct InputScene {
    pub window: WindowHandle<TestRoot>,
    pub dispatcher: Wired,
    pub cells: Rc<Cells>,
    pub parent: NodeId,
    pub input: NodeId,
    pub button: NodeId,
}

pub fn js_input_scene_of(cx: &mut TestAppContext, value: &str) -> InputScene {
    let rig = Rig::default();
    let [parent, input, button] = {
        let mut h = rig.cells.document.borrow_mut();
        let root = h.root();
        let [parent, input, button] =
            ["div", "input", "button"].map(|tag| h.create_node(tag).unwrap());
        h.append_child(&*rig.cells.window.borrow(), root, parent)
            .unwrap();
        h.append_child(&*rig.cells.window.borrow(), parent, input)
            .unwrap();
        h.append_child(&*rig.cells.window.borrow(), parent, button)
            .unwrap();
        h.set_style_prop(input, "width", 100.0).unwrap();
        h.set_style_prop(input, "text_size", 10.0).unwrap();
        h.mark_tab_dirty(input);
        h.mark_tab_dirty(button);
        h.inputs
            .get_mut()
            .entry(input)
            .or_default()
            .set_value(value);
        [parent, input, button]
    };
    let engine = wiring::engine_for(&rig);
    engine.eval::<()>("globalThis.out = [];").unwrap();
    let (report, _reported) = capture_reporter();
    let dispatcher = rig.dispatcher(&engine, report);
    let window = inca_window::test_support::mount_active(cx, dispatcher.dispatcher(), parent);
    InputScene {
        window,
        dispatcher,
        cells: Rc::clone(&rig.cells),
        parent,
        input,
        button,
    }
}

impl InputScene {
    pub fn in_window<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&Self, &mut Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.window.into(), |_, window, cx| f(self, window, cx))
            .unwrap()
    }

    pub fn draw(&self, cx: &mut TestAppContext) {
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
    }

    pub fn focus(&self, cx: &mut TestAppContext, node: NodeId) {
        self.cells.document.borrow_mut().request_focus(node, false);
        self.draw(cx);
        cx.run_until_parked();
    }

    /// Dispatches the space-separated keystrokes and returns whether the last one was handled.
    pub fn key(&self, cx: &mut TestAppContext, keys: &str) -> bool {
        let mut handled = false;
        for key in keys.split(' ') {
            let keystroke =
                Keystroke::parse_with_os(key, cx.read(|cx| cx.platform_info().os)).unwrap();
            handled = self.in_window(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx));
        }
        cx.run_until_parked();
        self.draw(cx);
        handled
    }

    pub fn request(&self, cx: &mut TestAppContext, request: InputRequest) {
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .dispatcher()
                .input_request(scene.input, request, window, cx);
        });
    }

    pub fn state(&self) -> inca_document::InputState {
        self.cells.document.borrow().inputs.borrow()[&self.input].clone()
    }

    pub fn set_attribute(&self, name: &str) {
        let mut document = self.cells.document.borrow_mut();
        document.set_attribute(self.input, name, true).unwrap();
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
pub fn record(s: &InputScene, node: NodeId, events: &[&str]) {
    s.dispatcher
        .engine
        .eval::<()>(&LOG.replace("INPUT", &s.input.to_string()))
        .unwrap();
    for event in events {
        listen(&s.dispatcher, node, event, "rec");
    }
}

/// Takes the rows logged so far.
pub fn log(s: &InputScene) -> Vec<serde_json::Value> {
    let json = s
        .dispatcher
        .engine
        .eval::<String>("JSON.stringify(globalThis.log.splice(0))")
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

/// Takes the types of the rows logged so far.
pub fn types(s: &InputScene) -> Vec<String> {
    log(s)
        .iter()
        .map(|row| row[0].as_str().unwrap().to_owned())
        .collect()
}

/// Cancels each `event` at `node`.
pub fn cancel(s: &InputScene, node: NodeId, event: &str) {
    listen(&s.dispatcher, node, event, "(e) => e.preventDefault()");
}

pub fn cancel_beforeinput(s: &InputScene) {
    cancel(s, s.input, "beforeinput");
}

/// Runs `js` with `INPUT` replaced by the id of the input.
pub fn eval_input(s: &InputScene, js: &str) {
    s.dispatcher
        .engine
        .eval::<()>(&js.replace("INPUT", &s.input.to_string()))
        .unwrap();
}

pub fn replace(text: &str) -> InputRequest {
    InputRequest::Replace {
        range: None,
        text: text.to_owned(),
    }
}

pub fn mark(text: &str) -> InputRequest {
    InputRequest::Mark {
        range: None,
        text: text.to_owned(),
        selected: None,
    }
}

pub fn row(
    kind: &str,
    input_type: Option<&str>,
    data: Option<&str>,
    composing: Option<bool>,
    cancelable: bool,
    value: &str,
) -> serde_json::Value {
    serde_json::json!([kind, input_type, data, composing, cancelable, value])
}

/// The `keydown` row of a key typed at an input holding `value`.
pub fn keydown(composing: bool, value: &str) -> serde_json::Value {
    row("keydown", None, None, Some(composing), true, value)
}

/// Dispatch helpers that put JS-visible work on the scene.
pub fn js_input_scene(cx: &mut TestAppContext, value: &str) -> InputScene {
    js_input_scene_of(cx, value)
}

/// The value and composition of the input.
pub fn edited_text(s: &InputScene) -> (String, Option<std::ops::Range<usize>>) {
    let state = s.state();
    (state.value().to_owned(), state.composition())
}
