// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The test scene of the input dispatch: an `input` and a `button` in an active window, and the
//! helpers that log and compare the events they receive.

#![allow(clippy::unwrap_used)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{App, AppContext as _, Keystroke, TestAppContext, WindowHandle};
use inca_document::InputState;
use inca_document::{EventPayload, EventTargetId, MousePayload, flags_of};
use inca_gpui::{EventSink, InputRequest};
use inca_node::NodeId;
use serde_json::{Value, json};

use crate::test_support::{TestRoot, cells};
use crate::tests::{Walked, fake_dispatcher_on};
use crate::{Cells, EventDispatcher};

type Rule = Box<dyn Fn(&Walked) -> bool>;

/// A walk the fake script took and the value of the input when it did.
struct Logged {
    walked: Walked,
    value: String,
}

/// An `input` holding a value (100 px wide, 6 px per char, from x 0) and a `button`, under a `div`
/// in an active window. [`input_scene`] runs the dispatch on a fake script.
pub(crate) struct InputScene {
    pub(crate) window: WindowHandle<TestRoot>,
    pub(crate) dispatcher: EventDispatcher,
    pub(crate) cells: Rc<Cells>,
    pub(crate) parent: NodeId,
    pub(crate) input: NodeId,
    pub(crate) button: NodeId,
    logged: Rc<RefCell<Vec<Logged>>>,
    recorded: RefCell<Option<(NodeId, Vec<String>)>>,
    window_blur: Cell<bool>,
    rules: Rc<RefCell<Vec<Rule>>>,
}

/// The nodes of a scene, with `value` in the input.
fn tree(cells: &Cells, value: &str) -> [NodeId; 3] {
    let mut h = cells.document.borrow_mut();
    let root = h.root();
    let [parent, input, button] = ["div", "input", "button"].map(|tag| h.create_node(tag).unwrap());
    h.append_child(&*cells.window.borrow(), root, parent)
        .unwrap();
    h.append_child(&*cells.window.borrow(), parent, input)
        .unwrap();
    h.append_child(&*cells.window.borrow(), parent, button)
        .unwrap();
    h.set_style_prop(input, "width", 100.0).unwrap();
    h.set_style_prop(input, "text_size", 10.0).unwrap();
    h.mark_tab_dirty(input);
    h.mark_tab_dirty(button);
    h.set_input_value(input, value);
    [parent, input, button]
}

/// A scene whose script is a fake.
pub(crate) fn input_scene(cx: &mut TestAppContext, value: &str) -> InputScene {
    let (cells, _wake) = cells();
    let [parent, input, button] = tree(&cells, value);
    let (dispatcher, fake) = fake_dispatcher_on(&cells);
    let logged: Rc<RefCell<Vec<Logged>>> = Rc::default();
    let rules: Rc<RefCell<Vec<Rule>>> = Rc::default();
    {
        let (cells, logged, rules) = (Rc::clone(&cells), Rc::clone(&logged), Rc::clone(&rules));
        fake.on_walk(move |walked| {
            let value = {
                let document = cells.document.borrow();
                document
                    .input(input)
                    .map_or_else(String::new, |state| state.value().to_owned())
            };
            logged.borrow_mut().push(Logged {
                walked: walked.clone(),
                value,
            });
            let rules = rules.borrow();
            rules
                .iter()
                .fold(false, |canceled, rule| rule(walked) | canceled)
        });
    }
    let window = cx.add_window(|_, _| TestRoot {
        cells: Rc::clone(&cells),
        node: parent,
        dispatcher: dispatcher.clone(),
    });
    cx.update_window(window.into(), |_, window, cx| {
        window.draw(cx).clear(cx);
        window.activate_window();
    })
    .unwrap();
    InputScene {
        window,
        dispatcher,
        cells,
        parent,
        input,
        button,
        logged,
        recorded: RefCell::default(),
        window_blur: Cell::new(false),
        rules,
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
        self.cells.document.borrow_mut().request_focus(node, false);
        self.draw(cx);
        cx.run_until_parked();
    }

    /// Dispatches the space-separated keystrokes and returns whether the last one was handled.
    pub(crate) fn key(&self, cx: &mut TestAppContext, keys: &str) -> bool {
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
        self.cells
            .document
            .borrow()
            .input(self.input)
            .unwrap()
            .clone()
    }

    /// Selects `anchor` to `focus` in the input.
    pub(crate) fn select(&self, anchor: usize, focus: usize) {
        let mut document = self.cells.document.borrow_mut();
        document.edit_input(self.input, |state| state.select(anchor, focus));
    }

    /// Writes `value` into the input, which `setInputValue` does for a script.
    pub(crate) fn write_value(&self, value: &str) {
        write_value(&self.cells, self.input, value);
    }

    pub(crate) fn set_attribute(&self, name: &str) {
        let mut document = self.cells.document.borrow_mut();
        document.set_attribute(self.input, name, true).unwrap();
    }
}

/// Starts the log and records `events` at `node`.
pub(crate) fn record(s: &InputScene, node: NodeId, events: &[&str]) {
    s.logged.borrow_mut().clear();
    let events = events.iter().map(|&event| event.to_owned()).collect();
    *s.recorded.borrow_mut() = Some((node, events));
}

/// Adds a `["window blur"]` row to the log for each `blur` at the window.
pub(crate) fn record_window_blur(s: &InputScene) {
    s.window_blur.set(true);
}

/// The row of a walk: `[type, inputType, data, isComposing, cancelable, value]`, the fields an
/// event has not read as `null`.
fn row_of(logged: &Logged) -> Value {
    let walked = &logged.walked;
    let (input_type, data, composing, kind) = match &walked.payload {
        EventPayload::Input(input) => (
            Some(input.input_type.as_str()),
            input.data.as_deref(),
            Some(input.is_composing),
            Some(input.input_type),
        ),
        EventPayload::Composition { data } => (None, Some(data.as_str()), None, None),
        EventPayload::Key(key) => (None, None, Some(key.is_composing), None),
        _ => (None, None, None, None),
    };
    let cancelable = flags_of(&walked.name, kind).cancelable;
    row(
        &walked.name,
        input_type,
        data,
        composing,
        cancelable,
        &logged.value,
    )
}

/// Takes the rows logged so far.
pub(crate) fn log(s: &InputScene) -> Vec<Value> {
    let logged = std::mem::take(&mut *s.logged.borrow_mut());
    let recorded = s.recorded.borrow();
    logged
        .iter()
        .filter_map(|entry| {
            let walked = &entry.walked;
            if s.window_blur.get()
                && walked.target == EventTargetId::Window
                && walked.name == "blur"
            {
                return Some(json!(["window blur"]));
            }
            let (node, events) = recorded.as_ref()?;
            let hit = walked.target == EventTargetId::Node(*node) && events.contains(&walked.name);
            hit.then(|| row_of(entry))
        })
        .collect()
}

/// Takes the types of the rows logged so far.
pub(crate) fn types(s: &InputScene) -> Vec<String> {
    log(s)
        .iter()
        .map(|row| row[0].as_str().unwrap().to_owned())
        .collect()
}

/// The `type@target` of every walk the fake script took since the scene was made or [`record`]
/// last ran, whatever its target.
pub(crate) fn walked(s: &InputScene) -> Vec<String> {
    s.logged
        .borrow()
        .iter()
        .map(|entry| entry.walked.label())
        .collect()
}

/// Forgets the walks of the fake script so far.
pub(crate) fn forget(s: &InputScene) {
    s.logged.borrow_mut().clear();
}

/// The nodes the first `event` walk skipped.
pub(crate) fn cut_of(s: &InputScene, event: &str) -> Vec<NodeId> {
    let logged = s.logged.borrow();
    let entry = logged.iter().find(|entry| entry.walked.name == event);
    entry.unwrap().walked.cut.clone()
}

/// Runs `rule` on each walk of the fake script, which cancels the event when it returns true.
pub(crate) fn on_walk(s: &InputScene, rule: impl Fn(&Walked) -> bool + 'static) {
    s.rules.borrow_mut().push(Box::new(rule));
}

/// Runs `act` on the cells at each `event` walk at `node`, as a listener that calls a native does.
pub(crate) fn act_at(
    s: &InputScene,
    node: NodeId,
    event: &'static str,
    act: impl Fn(&Cells) + 'static,
) {
    let cells = Rc::clone(&s.cells);
    on_walk(s, move |walked| {
        if walked.name == event && walked.target == EventTargetId::Node(node) {
            act(&cells);
        }
        false
    });
}

/// Cancels each `event` at `node`.
pub(crate) fn cancel(s: &InputScene, node: NodeId, event: &str) {
    let event = event.to_owned();
    on_walk(s, move |walked| {
        walked.name == event && walked.target == EventTargetId::Node(node)
    });
}

/// Writes `value` into the text state of `input`, which `setInputValue` does for a script.
pub(crate) fn write_value(cells: &Cells, input: NodeId, value: &str) {
    cells.document.borrow_mut().set_input_value(input, value);
}

/// A mouse payload of `button` at window x `x` and y 5.
pub(crate) fn mouse_of(x: f32, button: u8, shift: bool) -> MousePayload {
    let modifiers = inca_document::Modifiers::none().set(inca_document::Modifiers::SHIFT, shift);
    MousePayload {
        button,
        ..MousePayload::at(x, 5.0, modifiers)
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

pub(crate) fn cancel_beforeinput(s: &InputScene) {
    cancel(s, s.input, "beforeinput");
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
