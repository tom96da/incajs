// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scene the key tests run on.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{App, AppContext as _, Keystroke, TestAppContext, Window, WindowHandle, WindowId};
use inca_document::{EventPayload, EventTargetId, KeyPayload};
use inca_gpui::{EventSink, InputRequest};
use inca_node::NodeId;
use inca_style::set_style;

use super::{FakeScript, Walked, fake_dispatcher_on, mouse_at};
use crate::test_support::{TestRoot, cells};
use crate::{Cells, EventDispatcher};

type Rule = Box<dyn Fn(&Walked) -> bool>;

/// Two buttons, a focusable div (`field`) and a div without a tab index (`plain`) under one
/// parent. [`scene`] and [`input_scene`] run the dispatch on a [`FakeScript`]. A listener
/// of the node cancels each key event while `cancel` is set, and a listener of the parent
/// while `cancel_parent` is set.
pub(crate) struct Scene {
    pub(crate) window: WindowHandle<TestRoot>,
    pub(crate) dispatcher: EventDispatcher,
    pub(crate) cells: Rc<Cells>,
    pub(crate) parent: NodeId,
    pub(crate) first: NodeId,
    pub(crate) second: NodeId,
    pub(crate) field: NodeId,
    pub(crate) plain: NodeId,
    fake: Rc<FakeScript>,
    cancel: Rc<Cell<bool>>,
    cancel_parent: Rc<Cell<bool>>,
    rules: Rc<RefCell<Vec<Rule>>>,
    watched: RefCell<Vec<NodeId>>,
    seen: RefCell<Vec<Walked>>,
}

/// The nodes of a [`Scene`]. The field is an `input` holding `value` for `Some(value)`.
fn tree(cells: &Cells, value: Option<&str>) -> [NodeId; 5] {
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

/// A scene whose script is a [`FakeScript`].
pub(crate) fn scene(cx: &mut TestAppContext) -> Scene {
    fake_scene(cx, None)
}

/// [`scene`] with an `input` holding `value` as the field.
pub(crate) fn input_scene(cx: &mut TestAppContext, value: &str) -> Scene {
    fake_scene(cx, Some(value))
}

fn fake_scene(cx: &mut TestAppContext, value: Option<&str>) -> Scene {
    let (cells, _wake) = cells();
    let [parent, first, second, field, plain] = tree(&cells, value);
    let (dispatcher, fake) = fake_dispatcher_on(&cells);
    let (cancel, cancel_parent) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let rules: Rc<RefCell<Vec<Rule>>> = Rc::default();
    {
        let (cancel, cancel_parent, rules) = (
            Rc::clone(&cancel),
            Rc::clone(&cancel_parent),
            Rc::clone(&rules),
        );
        fake.on_walk(move |walked| {
            let key = matches!(walked.name.as_str(), "keydown" | "keyup");
            let mut canceled = key && (cancel.get() || cancel_parent.get());
            for rule in rules.borrow().iter() {
                canceled |= rule(walked);
            }
            canceled
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
    Scene {
        window,
        dispatcher,
        cells,
        parent,
        first,
        second,
        field,
        plain,
        fake,
        cancel,
        cancel_parent,
        rules,
        watched: RefCell::new(vec![first, second, field]),
        seen: RefCell::default(),
    }
}

impl Scene {
    pub(crate) fn focus(&self, cx: &mut TestAppContext, node: NodeId) {
        self.cells
            .document
            .borrow_mut()
            .focus
            .request_focus(node, false);
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    pub(crate) fn blur(&self, cx: &mut TestAppContext, node: NodeId) {
        self.cells.document.borrow_mut().focus.request_blur(node);
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    pub(crate) fn focused(&self, cx: &mut TestAppContext) -> Option<NodeId> {
        self.in_window(cx, |scene, window, cx| scene.cells.focused_node(window, cx))
    }

    pub(crate) fn key(&self, cx: &mut TestAppContext, key: &str, up: bool, canceled: bool) {
        let keystroke = Keystroke::parse_with_os(key, cx.read(|cx| cx.platform_info().os)).unwrap();
        let payload = EventPayload::Key(KeyPayload {
            key: keystroke.key.clone(),
            repeat: false,
            modifiers: inca_gpui::modifiers_of(keystroke.modifiers),
            is_composing: false,
        });
        self.cancel.set(canceled);
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .key_input(&keystroke, up, &payload, window, cx);
        });
        cx.run_until_parked();
    }

    /// Sends the space-separated keystrokes through the window, so a
    /// typed character reaches the input.
    pub(crate) fn press(&self, cx: &mut TestAppContext, keys: &str) {
        for key in keys.split(' ') {
            let keystroke =
                Keystroke::parse_with_os(key, cx.read(|cx| cx.platform_info().os)).unwrap();
            self.in_window(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx));
        }
        cx.run_until_parked();
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
    }

    /// Sends the release of `key` through the window.
    pub(crate) fn release(&self, cx: &mut TestAppContext, key: &str) {
        let keystroke = Keystroke::parse_with_os(key, cx.read(|cx| cx.platform_info().os))
            .unwrap()
            .with_simulated_ime();
        self.in_window(cx, |_, window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::KeyUp(gpui::KeyUpEvent { keystroke }),
                cx,
            );
        });
        cx.run_until_parked();
    }

    /// Sends `request` to the input.
    pub(crate) fn request(&self, cx: &mut TestAppContext, request: InputRequest) {
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .input_request(scene.field, request, window, cx);
        });
    }

    /// The text state of the input field.
    pub(crate) fn state(&self) -> inca_document::InputState {
        self.cells.document.borrow().inputs.borrow()[&self.field].clone()
    }

    pub(crate) fn mouse_down(&self, cx: &mut TestAppContext, node: NodeId) {
        let payload = mouse_at(1.0, 1.0);
        self.in_window(cx, |scene, window, cx| {
            let sink = &scene.dispatcher;
            sink.pointer_input("mousedown", node, &payload, window, cx);
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    pub(crate) fn fire(&self, cx: &mut TestAppContext, node: NodeId, event: &str) {
        self.in_window(cx, |scene, window, cx| {
            let payload = EventPayload::Focus {
                related_target: None,
            };
            EventSink::fire(&scene.dispatcher, node, event, &payload, window, cx);
        });
    }

    pub(crate) fn window_id(&self, cx: &mut TestAppContext) -> WindowId {
        self.in_window(cx, |_, window, _| window.window_handle().window_id())
    }

    pub(crate) fn in_window<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&Self, &mut Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.window.into(), |_, window, cx| f(self, window, cx))
            .unwrap()
    }

    /// Every walk since the scene was made or [`Self::forget`] was last called.
    pub(crate) fn walks(&self) -> Vec<Walked> {
        let mut seen = self.seen.borrow_mut();
        seen.extend(self.fake.take());
        seen.clone()
    }

    /// Drops the walks so far.
    pub(crate) fn forget(&self) {
        self.walks();
        self.seen.borrow_mut().clear();
    }

    /// The types of the walks at `node` that are one of `events`.
    pub(crate) fn types(&self, node: NodeId, events: &[&str]) -> Vec<String> {
        self.walks()
            .into_iter()
            .filter(|walked| {
                walked.target == EventTargetId::Node(node) && events.contains(&walked.name.as_str())
            })
            .map(|walked| walked.name)
            .collect()
    }

    /// The nodes of the `click` walks at the first, the second, the field and the nodes of
    /// [`Self::watch`].
    pub(crate) fn clicks(&self) -> Vec<NodeId> {
        let watched = self.watched.borrow();
        self.walks()
            .iter()
            .filter(|walked| walked.name == "click")
            .filter_map(|walked| match walked.target {
                EventTargetId::Node(node) if watched.contains(&node) => Some(node),
                _ => None,
            })
            .collect()
    }

    /// Makes [`Self::clicks`] report the clicks at `node` too.
    pub(crate) fn watch(&self, node: NodeId) {
        self.watched.borrow_mut().push(node);
    }

    /// Cancels each `name` walk at `node`.
    pub(crate) fn cancel_at(&self, node: NodeId, name: &str) {
        let name = name.to_owned();
        self.on_walk(move |walked| {
            walked.name == name && walked.target == EventTargetId::Node(node)
        });
    }

    /// Cancels the key events of the parent while `on` is set.
    pub(crate) fn cancel_at_parent(&self, on: bool) {
        self.cancel_parent.set(on);
    }

    /// Runs `rule` on each walk, which cancels the event when it returns true.
    pub(crate) fn on_walk(&self, rule: impl Fn(&Walked) -> bool + 'static) {
        self.rules.borrow_mut().push(Box::new(rule));
    }

    /// A function that queues a job on the script, which a rule can own.
    pub(crate) fn queue_handle(&self) -> impl Fn(Box<dyn FnOnce()>) + 'static {
        let fake = Rc::clone(&self.fake);
        move |job| fake.queue_job(job)
    }
}
