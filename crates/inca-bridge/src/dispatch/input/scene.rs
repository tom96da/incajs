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

use crate::test_support::{TestRoot, engine_for, mount_active};
use crate::tests::{FakeScript, Walked, fake_dispatcher_on, listen, listen_global};
use crate::{EventDispatcher, Host};
use inca_style::set_style;

type Rule = Box<dyn Fn(&Walked) -> bool>;

/// A walk the fake script took and the value of the input when it did.
struct Logged {
    walked: Walked,
    value: String,
}

/// An `input` holding a value (100 px wide, 6 px per char, from x 0) and a `button`, under a `div`
/// in an active window. [`input_scene`] runs the dispatch on a [`FakeScript`] and
/// [`js_input_scene`] on an engine, for the tests of what a native does to the tree and of what
/// JS reads off an event. [`record`] and [`log`] work on both.
pub(crate) struct InputScene {
    pub(crate) window: WindowHandle<TestRoot>,
    pub(crate) dispatcher: EventDispatcher,
    pub(crate) host: Rc<Host>,
    pub(crate) engine: Rc<inca_jsenv::Engine>,
    pub(crate) parent: NodeId,
    pub(crate) input: NodeId,
    pub(crate) button: NodeId,
    fake: Option<Rc<FakeScript>>,
    logged: Rc<RefCell<Vec<Logged>>>,
    recorded: RefCell<Option<(NodeId, Vec<String>)>>,
    window_blur: Cell<bool>,
    rules: Rc<RefCell<Vec<Rule>>>,
}

/// The nodes of a scene, with `value` in the input.
fn tree(host: &Host, value: &str) -> [NodeId; 3] {
    let mut h = host.document.borrow_mut();
    let root = h.root();
    let [parent, input, button] =
        ["div", "input", "button"].map(|tag| h.tree.create_node(tag).unwrap());
    h.tree.append_child(root, parent).unwrap();
    h.tree.append_child(parent, input).unwrap();
    h.tree.append_child(parent, button).unwrap();
    set_style(&mut h.tree, input, "width", 100.0).unwrap();
    set_style(&mut h.tree, input, "text_size", 10.0).unwrap();
    h.focus.mark_tab_dirty(input);
    h.focus.mark_tab_dirty(button);
    h.inputs
        .get_mut()
        .entry(input)
        .or_default()
        .set_value(value);
    [parent, input, button]
}

/// A scene whose script is a [`FakeScript`].
pub(crate) fn input_scene(cx: &mut TestAppContext, value: &str) -> InputScene {
    let host = Rc::new(Host::default());
    let [parent, input, button] = tree(&host, value);
    let (dispatcher, fake) = fake_dispatcher_on(&host);
    let logged: Rc<RefCell<Vec<Logged>>> = Rc::default();
    let rules: Rc<RefCell<Vec<Rule>>> = Rc::default();
    {
        let (host, logged, rules) = (Rc::clone(&host), Rc::clone(&logged), Rc::clone(&rules));
        fake.on_walk(move |walked| {
            let value = {
                let document = host.document.borrow();
                let inputs = document.inputs.borrow();
                inputs
                    .get(&input)
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
    let engine = Rc::clone(&dispatcher.engine);
    let window = cx.add_window(|_, _| TestRoot {
        host: Rc::clone(&host),
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
        host,
        engine,
        parent,
        input,
        button,
        fake: Some(fake),
        logged,
        recorded: RefCell::default(),
        window_blur: Cell::new(false),
        rules,
    }
}

/// A scene on an engine.
pub(crate) fn js_input_scene(cx: &mut TestAppContext, value: &str) -> InputScene {
    let host = Rc::new(Host::default());
    let [parent, input, button] = tree(&host, value);
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
        fake: None,
        logged: Rc::default(),
        recorded: RefCell::default(),
        window_blur: Cell::new(false),
        rules: Rc::default(),
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
        self.host
            .document
            .borrow_mut()
            .focus
            .request_focus(node, false);
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
        self.host.document.borrow().inputs.borrow()[&self.input].clone()
    }

    /// Selects `anchor` to `focus` in the input.
    pub(crate) fn select(&self, anchor: usize, focus: usize) {
        let host = self.host.document.borrow();
        let mut inputs = host.inputs.borrow_mut();
        inputs.get_mut(&self.input).unwrap().select(anchor, focus);
    }

    /// Writes `value` into the input, which `setInputValue` does for a script.
    pub(crate) fn write_value(&self, value: &str) {
        write_value(&self.host, self.input, value);
    }

    pub(crate) fn set_attribute(&self, name: &str) {
        let mut host = self.host.document.borrow_mut();
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
    if s.fake.is_some() {
        s.logged.borrow_mut().clear();
        let events = events.iter().map(|&event| event.to_owned()).collect();
        *s.recorded.borrow_mut() = Some((node, events));
        return;
    }
    s.engine
        .eval::<()>(&LOG.replace("INPUT", &s.input.to_string()))
        .unwrap();
    for event in events {
        listen(&s.dispatcher, node, event, "rec");
    }
}

/// Adds a `["window blur"]` row to the log for each `blur` at the window.
pub(crate) fn record_window_blur(s: &InputScene) {
    if s.fake.is_some() {
        s.window_blur.set(true);
        return;
    }
    listen_global(
        &s.dispatcher,
        "$window",
        "blur",
        false,
        "() => globalThis.log.push(['window blur'])",
    );
}

/// The row of a walk, as [`LOG`] writes it.
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
    if s.fake.is_some() {
        let logged = std::mem::take(&mut *s.logged.borrow_mut());
        let recorded = s.recorded.borrow();
        return logged
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
                let hit =
                    walked.target == EventTargetId::Node(*node) && events.contains(&walked.name);
                hit.then(|| row_of(entry))
            })
            .collect();
    }
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

/// Runs `act` on the host at each `event` walk at `node`, as a listener that calls a native does.
pub(crate) fn act_at(
    s: &InputScene,
    node: NodeId,
    event: &'static str,
    act: impl Fn(&Host) + 'static,
) {
    let host = Rc::clone(&s.host);
    on_walk(s, move |walked| {
        if walked.name == event && walked.target == EventTargetId::Node(node) {
            act(&host);
        }
        false
    });
}

/// Cancels each `event` at `node`.
pub(crate) fn cancel(s: &InputScene, node: NodeId, event: &str) {
    if s.fake.is_some() {
        let event = event.to_owned();
        on_walk(s, move |walked| {
            walked.name == event && walked.target == EventTargetId::Node(node)
        });
    } else {
        listen(&s.dispatcher, node, event, "(e) => e.preventDefault()");
    }
}

/// Writes `value` into the text state of `input`, which `setInputValue` does for a script.
pub(crate) fn write_value(host: &Host, input: NodeId, value: &str) {
    let document = host.document.borrow();
    let mut inputs = document.inputs.borrow_mut();
    inputs.entry(input).or_default().set_value(value);
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

/// Runs `js` with `INPUT` replaced by the id of the input.
pub(crate) fn eval_input(s: &InputScene, js: &str) {
    assert!(s.fake.is_none(), "eval_input needs a scene on an engine");
    s.engine
        .eval::<()>(&js.replace("INPUT", &s.input.to_string()))
        .unwrap();
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
