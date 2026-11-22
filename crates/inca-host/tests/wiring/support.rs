// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scenes the wiring tests run on: a window over a document, with a JS engine as the script of
//! the dispatch.

#![allow(clippy::unwrap_used, dead_code, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Keystroke, MouseButton, MouseUpEvent, TestAppContext, VisualTestContext,
    Window, WindowHandle, WindowId,
};
pub use inca_bridge::test_support::{HTML, MATHML, RECORDER, SVG};
use inca_bridge::test_support::{Reported, capture_reporter};
use inca_document::{EVENTS, PointerSource, WheelPayload};
use inca_document::{EventPayload, EventTargetId, KeyPayload, MousePayload};
use inca_gpui::{EventSink, InputLayout, InputRequest};
use inca_host::wiring::{self, Rig, Wired};
pub use inca_host::wiring::{
    callback_stored, dispatcher_on_clock, dispatcher_with_engine, engine_for, engine_on_clock,
    eval_json, fire_and_read, focusable_tree, host_on_executor_clock, listen, listen_global,
    listen_with, mount_active, mount_root, mount_with_timers, read_out, tagged,
};
use inca_node::NodeId;
use inca_style::set_style;
pub use inca_window::test_support::{
    Frame, TestRoot, add_sheet, apply_focus, at, draw, is_focused, key_payload, mouse_at,
    node_under_root, pointer_click, state, tree_of_three, wheel_at,
};
use inca_window::{Cells, EventDispatcher};

const POINTER_AND_MOUSE: [&str; 18] = [
    "pointerdown",
    "pointerup",
    "pointermove",
    "pointerover",
    "pointerout",
    "pointerenter",
    "pointerleave",
    "gotpointercapture",
    "lostpointercapture",
    "mousedown",
    "mouseup",
    "mousemove",
    "mouseover",
    "mouseout",
    "mouseenter",
    "mouseleave",
    "click",
    "auxclick",
];

/// Two nodes `a` and `b` under the document root. The listeners on `a` and `b` record
/// `type@target` into `globalThis.out`.
pub struct PointerScene {
    pub dispatcher: Wired,
    pub cells: Rc<Cells>,
    pub cx: VisualTestContext,
    pub a: NodeId,
    pub b: NodeId,
}

pub fn js_pointer_scene(cx: &mut TestAppContext) -> PointerScene {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let (a, b) = (node_under_root(&cells), node_under_root(&cells));
    let record = r"
        (e) => {
            if (e.eventPhase === 2) {
                out.push(`${e.type}@${e.target}`);
            }
        }
        ";
    for node in [a, b] {
        for event in POINTER_AND_MOUSE {
            listen(&dispatcher, node, event, record);
        }
    }
    PointerScene {
        dispatcher,
        cells,
        cx: cx.add_empty_window().clone(),
        a,
        b,
    }
}

fn button_payload(button: u8) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        button,
        ..MousePayload::at(1.0, 1.0, inca_document::Modifiers::none())
    })
}

impl PointerScene {
    /// The events at `a` and `b` since the last call.
    pub fn take(&self) -> Vec<String> {
        serde_json::from_value(eval_json(&self.dispatcher, "out.splice(0)")).unwrap()
    }

    pub fn listen(&self, node: NodeId, event: &str, f: &str) {
        listen(&self.dispatcher, node, event, f);
    }

    /// Reports the pointer over `node` as the hover events do.
    pub fn hover(&mut self, node: NodeId) {
        let sink = self.dispatcher.dispatcher().clone();
        self.cx
            .update(|window, cx| sink.hover_changed(node, true, window, cx));
        self.cx.run_until_parked();
    }

    /// A raw move over `node` by `(x, y)` from the previous position.
    pub fn moved(&mut self, node: NodeId, (x, y): (f32, f32)) {
        let sink = self.dispatcher.dispatcher().clone();
        self.cx.update(|window, cx| {
            sink.pointer_moved(at(x, y), cx);
            sink.pointer_over(node, window, cx);
            sink.pointer_input("mousemove", node, &mouse_at(x, y), window, cx);
            sink.hover_changed(node, true, window, cx);
        });
    }

    pub fn down(&mut self, node: NodeId, button: u8) {
        let sink = self.dispatcher.dispatcher().clone();
        self.cx.update(|window, cx| {
            sink.pointer_input("mousedown", node, &button_payload(button), window, cx);
        });
    }

    /// A release of `button` over `node`, pressed over `pressed`.
    pub fn up(&mut self, node: NodeId, pressed: NodeId, button: u8) {
        let sink = self.dispatcher.dispatcher().clone();
        let mouse_button =
            [MouseButton::Left, MouseButton::Middle, MouseButton::Right][button as usize];
        let event = MouseUpEvent {
            button: mouse_button,
            position: at(1.0, 1.0),
            modifiers: gpui::Modifiers::none(),
            click_count: 1,
        };
        self.cx.update(|window, cx| {
            sink.pointer_input("mouseup", node, &button_payload(button), window, cx);
            sink.pointer_released(&event, Some(pressed), node, window, cx);
        });
    }
}

/// Two buttons, a focusable div or an `input` (`field`) and a div without a tab index (`plain`)
/// under one parent, in an active window. The buttons and the field have a `click` listener that
/// pushes `click:currentTarget:isTrusted` into `out` and `keydown` and `keyup` listeners that read
/// `globalThis.cancel`.
pub struct KeyScene {
    pub window: WindowHandle<TestRoot>,
    pub dispatcher: Wired,
    pub cells: Rc<Cells>,
    pub parent: NodeId,
    pub first: NodeId,
    pub second: NodeId,
    pub field: NodeId,
    pub plain: NodeId,
}

fn key_tree(cells: &Cells, value: Option<&str>) -> [NodeId; 5] {
    let mut h = cells.document.borrow_mut();
    let root = h.root();
    let parent = h.tree.create_node("div").unwrap();
    h.tree.append_child(root, parent).unwrap();
    let tags = [
        "button",
        "button",
        if value.is_some() { "input" } else { "div" },
        "div",
    ];
    let [first, second, field, plain] = tags.map(|tag| {
        let node = h.tree.create_node(tag).unwrap();
        h.tree.append_child(parent, node).unwrap();
        node
    });
    match value {
        Some(value) => {
            set_style(&mut h.tree, field, "width", 100.0).unwrap();
            set_style(&mut h.tree, field, "text_size", 10.0).unwrap();
            h.inputs
                .get_mut()
                .entry(field)
                .or_default()
                .set_value(value);
        }
        None => h.tree.set_attribute(field, "tabindex", "0").unwrap(),
    }
    for node in [first, second, field] {
        h.focus.mark_tab_dirty(node);
    }
    [parent, first, second, field, plain]
}

pub type Scene = KeyScene;

pub fn js_scene(cx: &mut TestAppContext) -> KeyScene {
    js_scene_of(cx, None)
}

/// [`js_scene`] with an `input` holding `value` as the field.
pub fn js_key_input_scene(cx: &mut TestAppContext, value: &str) -> KeyScene {
    js_scene_of(cx, Some(value))
}

fn js_scene_of(cx: &mut TestAppContext, value: Option<&str>) -> KeyScene {
    let rig = Rig::default();
    let [parent, first, second, field, plain] = key_tree(&rig.cells, value);
    let engine = wiring::engine_for(&rig);
    engine.eval::<()>("globalThis.out = [];").unwrap();
    for button in [first, second, field] {
        engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({button}, 'click', (e) => {{
                    out.push(`click:${{e.currentTarget}}:${{e.isTrusted}}`);
                }});
                "
            ))
            .unwrap();
    }
    for node in [first, second, field] {
        for event in ["keydown", "keyup"] {
            engine
                .eval::<()>(&format!(
                    r"
                    __inca_test_listen__({node}, '{event}', (e) => {{
                        if (globalThis.cancel) {{
                            e.preventDefault();
                        }}
                    }});
                    "
                ))
                .unwrap();
        }
    }
    let (report, _reported) = capture_reporter();
    let dispatcher = rig.dispatcher(&engine, report);
    let window = inca_window::test_support::mount_active(cx, dispatcher.dispatcher(), parent);
    KeyScene {
        window,
        dispatcher,
        cells: Rc::clone(&rig.cells),
        parent,
        first,
        second,
        field,
        plain,
    }
}

impl KeyScene {
    pub fn focus(&self, cx: &mut TestAppContext, node: NodeId) {
        self.cells
            .document
            .borrow_mut()
            .focus
            .request_focus(node, false);
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    pub fn focused(&self, cx: &mut TestAppContext) -> Option<NodeId> {
        self.in_window(cx, |scene, window, cx| scene.cells.focused_node(window, cx))
    }

    pub fn key(&self, cx: &mut TestAppContext, key: &str, up: bool, canceled: bool) {
        let keystroke = Keystroke::parse_with_os(key, cx.read(|cx| cx.platform_info().os)).unwrap();
        let payload = EventPayload::Key(KeyPayload {
            key: keystroke.key.clone(),
            repeat: false,
            modifiers: inca_gpui::modifiers_of(keystroke.modifiers),
            is_composing: false,
        });
        self.dispatcher
            .engine
            .eval::<()>(&format!("globalThis.cancel = {canceled};"))
            .unwrap();
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .dispatcher()
                .key_input(&keystroke, up, &payload, window, cx);
        });
        cx.run_until_parked();
    }

    /// Sends the space-separated keystrokes through the window.
    pub fn press(&self, cx: &mut TestAppContext, keys: &str) {
        for key in keys.split(' ') {
            let keystroke =
                Keystroke::parse_with_os(key, cx.read(|cx| cx.platform_info().os)).unwrap();
            self.in_window(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx));
        }
        cx.run_until_parked();
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
    }

    pub fn request(&self, cx: &mut TestAppContext, request: InputRequest) {
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .dispatcher()
                .input_request(scene.field, request, window, cx);
        });
    }

    pub fn state(&self) -> inca_document::InputState {
        self.cells.document.borrow().inputs.borrow()[&self.field].clone()
    }

    pub fn in_window<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&Self, &mut Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.window.into(), |_, window, cx| f(self, window, cx))
            .unwrap()
    }
}

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
        self.cells
            .document
            .borrow_mut()
            .focus
            .request_focus(node, false);
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
        document.tree.set_attribute(self.input, name, true).unwrap();
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

/// A mouse payload of `button` at window x `x` and y 5.
pub fn mouse_of(x: f32, button: u8, shift: bool) -> MousePayload {
    let modifiers = inca_document::Modifiers::none().set(inca_document::Modifiers::SHIFT, shift);
    MousePayload {
        button,
        ..MousePayload::at(x, 5.0, modifiers)
    }
}

/// Dispatch helpers that put JS-visible work on the scene.
pub fn js_input_scene(cx: &mut TestAppContext, value: &str) -> InputScene {
    js_input_scene_of(cx, value)
}

pub fn seen(dispatcher: &Wired) -> serde_json::Value {
    let json = dispatcher
        .engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

pub fn seen_rows(dispatcher: &Wired) -> Vec<Vec<serde_json::Value>> {
    serde_json::from_value(seen(dispatcher)).unwrap()
}

/// Gives every node one object `{ node }` and counts the calls.
pub const HOOK: &str = r"
    globalThis.calls = 0;
    globalThis.objs = {};
    globalThis.nodeOf = (o) => (o === null ? null : o.node);
    globalThis.__inca_targets__ = (ids) => {
        globalThis.calls += 1;
        return ids.map((id) => (globalThis.objs[id] ??= { node: id }));
    };
    ";

/// Records `[label, target, currentTarget, eventPhase, path]` of the event it hears.
pub fn note_walk(label: &str) -> String {
    format!(
        r"
        (e) => out.push([
            '{label}',
            nodeOf(e.target),
            nodeOf(e.currentTarget),
            e.eventPhase,
            e.composedPath().map(nodeOf),
        ])
        "
    )
}

pub fn mouse_button(button: u8) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        button,
        buttons: inca_document::buttons_mask(button),
        ..MousePayload::at(1.0, 1.0, inca_document::Modifiers::none())
    })
}

pub const PATHS: &str = r"
    globalThis.seen = [];
    globalThis.paths = (e) => {
        globalThis.seen.push([e.target, e.currentTarget, e.eventPhase,
            e.composedPath()]); globalThis.kept = e; };
    ";

pub fn plain_mouse() -> MousePayload {
    MousePayload::at(7.0, 9.0, inca_document::Modifiers::none())
}

pub fn pointer_payload(source: PointerSource) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        pointer: Some(source),
        ..plain_mouse()
    })
}

pub fn wheel_payload() -> EventPayload {
    EventPayload::Wheel(WheelPayload {
        mouse: plain_mouse(),
        delta_x: 0.0,
        delta_y: 0.0,
        delta_z: 0.0,
        delta_mode: 0,
    })
}

/// The value and composition of the input.
pub fn edited_text(s: &InputScene) -> (String, Option<std::ops::Range<usize>>) {
    let state = s.state();
    (state.value().to_owned(), state.composition())
}

impl KeyScene {
    /// Moves `plain` between `first` and `second`.
    pub fn plain_between_the_buttons(&self) {
        self.cells
            .document
            .borrow_mut()
            .tree
            .insert_before(self.parent, self.plain, Some(self.second))
            .unwrap();
    }

    pub fn mouse_down(&self, cx: &mut TestAppContext, node: NodeId) {
        let payload = mouse_at(1.0, 1.0);
        self.in_window(cx, |scene, window, cx| {
            let sink = scene.dispatcher.dispatcher();
            sink.pointer_input("mousedown", node, &payload, window, cx);
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    /// A node of `namespace` named `tag` with `attributes`, before the field.
    pub fn add_link(&self, namespace: &str, tag: &str, attributes: &[(&str, &str)]) -> NodeId {
        let mut document = self.cells.document.borrow_mut();
        let node = document
            .tree
            .create_element_ns(Some(namespace), None, tag)
            .unwrap();
        for &(key, value) in attributes {
            document.tree.set_attribute(node, key, value).unwrap();
        }
        document
            .tree
            .insert_before(self.parent, node, Some(self.field))
            .unwrap();
        document.focus.mark_tab_dirty(node);
        node
    }
}

const SEEN: &str = r"
    globalThis.seen = [];
    globalThis.mark = (e) => { globalThis.seen.push(e.type + ':' + e.target); };
    ";

/// A focusable node under the root whose `focus` and `blur` are recorded into `seen`.
pub fn focusable_node(dispatcher: &Wired, cells: &Rc<Cells>) -> NodeId {
    let mut document = cells.document.borrow_mut();
    let node = document.tree.create_node("div").unwrap();
    let root = document.root();
    document.tree.append_child(root, node).unwrap();
    document
        .tree
        .set_attribute(node, "tabindex", inca_node::AttributeValue::from("0"))
        .unwrap();
    document.focus.mark_tab_dirty(node);
    drop(document);
    dispatcher.engine.eval::<()>(SEEN).unwrap();
    for event in ["focus", "blur"] {
        listen(dispatcher, node, event, "mark");
    }
    node
}
