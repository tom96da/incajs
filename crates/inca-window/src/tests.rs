// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the unit tests of several modules.

#![allow(clippy::unwrap_used)]

use std::cell::Cell;
use std::rc::Rc;

use gpui::{AppContext as _, TestAppContext};
use inca_document::{EventPayload, EventTargetId, MousePayload};
use inca_gpui::EventSink;
use inca_node::NodeId;

use crate::{Cells, EventDispatcher};
mod fake_script;

pub(crate) use fake_script::*;
pub(crate) use focus::*;

/// The HTML namespace.
pub(crate) const HTML: &str = "http://www.w3.org/1999/xhtml";

/// The SVG namespace.
pub(crate) const SVG: &str = "http://www.w3.org/2000/svg";

/// The `MathML` namespace.
pub(crate) const MATHML: &str = "http://www.w3.org/1998/Math/MathML";

pub(crate) mod focus;
pub(crate) mod input;
pub(crate) mod keys;

pub(crate) mod pointer {
    use gpui::{MouseButton, MouseUpEvent, VisualTestContext};

    use super::*;

    /// Two nodes `a` and `b` under the document root, with the dispatch running on a
    /// [`FakeScript`].
    pub(crate) struct Scene {
        pub(crate) dispatcher: EventDispatcher,
        pub(crate) cells: Rc<Cells>,
        pub(crate) fake: Rc<FakeScript>,
        pub(crate) cx: VisualTestContext,
        pub(crate) a: NodeId,
        pub(crate) b: NodeId,
        watched: Vec<NodeId>,
    }

    pub(crate) fn scene(cx: &mut TestAppContext) -> Scene {
        let (dispatcher, cells, fake) = fake_dispatcher();
        let (a, b) = (node_under_root(&cells), node_under_root(&cells));
        Scene {
            dispatcher,
            cells,
            fake,
            cx: cx.add_empty_window().clone(),
            a,
            b,
            watched: vec![a, b],
        }
    }

    impl Scene {
        /// The events at `a` and `b` and at the nodes of [`Self::watch`] since the last call.
        pub(crate) fn take(&self) -> Vec<String> {
            self.fake
                .take()
                .iter()
                .filter(|walked| {
                    matches!(walked.target, EventTargetId::Node(node) if self.watched.contains(&node))
                })
                .map(Walked::label)
                .collect()
        }

        /// The walks since the last call, at every node.
        pub(crate) fn take_walks(&self) -> Vec<Walked> {
            self.fake.take()
        }

        /// Makes [`Self::take`] report the events at `node` too.
        pub(crate) fn watch(&mut self, node: NodeId) {
            self.watched.push(node);
        }

        /// Cancels each event of type `name` while `cancel` is set.
        pub(crate) fn cancel(&self, name: &'static str, cancel: Rc<Cell<bool>>) {
            self.fake
                .on_walk(move |walked| walked.name == name && cancel.get());
        }

        /// Makes the `pointerdown` walk capture the pointer for `node`.
        pub(crate) fn capture_on_pointerdown(&self, node: NodeId) {
            let cells = Rc::clone(&self.cells);
            self.fake.on_walk(move |walked| {
                if walked.name == "pointerdown" {
                    request_capture(&cells, node);
                }
                false
            });
        }

        /// Runs `f` on each walk, and cancels the event when it returns true.
        pub(crate) fn on_walk(&self, f: impl Fn(&Walked) -> bool + 'static) {
            self.fake.on_walk(f);
        }

        /// Reports the pointer over `node` as the hover events do.
        pub(crate) fn hover(&mut self, node: NodeId) {
            let sink = self.dispatcher.clone();
            self.cx
                .update(|window, cx| sink.report_hover(Some(node), window, cx));
            *self.dispatcher.state.hovered.borrow_mut() = [node].into();
        }

        /// A raw move over `node` by `(x, y)` from the previous position.
        pub(crate) fn moved(&mut self, node: NodeId, (x, y): (f32, f32)) {
            let sink = self.dispatcher.clone();
            self.cx.update(|window, cx| {
                sink.pointer_moved(at(x, y), cx);
                sink.pointer_over(node, window, cx);
                sink.pointer_input("mousemove", node, &mouse_at(x, y), window, cx);
            });
            *self.dispatcher.state.hovered.borrow_mut() = [node].into();
        }

        /// A raw move over `node` to `(x, y)` with the left button held.
        pub(crate) fn moved_held(&mut self, node: NodeId, (x, y): (f32, f32)) {
            let sink = self.dispatcher.clone();
            let payload = EventPayload::Mouse(MousePayload {
                buttons: 1,
                ..MousePayload::at(x, y, inca_document::Modifiers::none())
            });
            self.cx.update(|window, cx| {
                sink.pointer_moved(at(x, y), cx);
                sink.pointer_over(node, window, cx);
                sink.pointer_input("mousemove", node, &payload, window, cx);
            });
        }

        /// Starts (`active`) or ends the drag of a thumb of the container `node`.
        pub(crate) fn bar_drag(&mut self, node: NodeId, active: bool) {
            let sink = self.dispatcher.clone();
            self.cx
                .update(|window, cx| sink.bar_drag(node, active, window, cx));
        }

        pub(crate) fn down(&mut self, node: NodeId, button: u8) {
            let sink = self.dispatcher.clone();
            self.cx.update(|window, cx| {
                sink.pointer_input("mousedown", node, &button_payload(button), window, cx);
            });
        }

        /// A press of `button` on a scrollbar of the container `node`.
        pub(crate) fn bar_down(&mut self, node: NodeId, button: u8) {
            let sink = self.dispatcher.clone();
            self.cx.update(|window, cx| {
                sink.bar_pressed(node, &button_payload(button), window, cx);
            });
        }

        /// A release of `button` after a press on a scrollbar of the container `node`.
        pub(crate) fn bar_up(&mut self, node: NodeId, button: u8) {
            let sink = self.dispatcher.clone();
            self.cx.update(|window, cx| {
                sink.bar_released(node, &button_payload(button), window, cx);
            });
        }

        /// A release of `button` over `node`, pressed over `pressed`.
        pub(crate) fn up(&mut self, node: NodeId, pressed: NodeId, button: u8) {
            self.up_counted(node, pressed, button, 1);
        }

        /// [`Self::up`] for the `count`-th click of a series.
        pub(crate) fn up_counted(
            &mut self,
            node: NodeId,
            pressed: NodeId,
            button: u8,
            count: usize,
        ) {
            let sink = self.dispatcher.clone();
            let mouse_button =
                [MouseButton::Left, MouseButton::Middle, MouseButton::Right][button as usize];
            let event = MouseUpEvent {
                button: mouse_button,
                position: at(1.0, 1.0),
                modifiers: gpui::Modifiers::none(),
                click_count: count,
            };
            self.cx.update(|window, cx| {
                sink.pointer_input("mouseup", node, &button_payload(button), window, cx);
                sink.pointer_released(&event, Some(pressed), node, window, cx);
            });
        }

        /// A script asking to capture the pointer for `node`.
        pub(crate) fn set_capture(&self, node: NodeId) {
            request_capture(&self.cells, node);
        }

        /// A script releasing the capture of `node`.
        pub(crate) fn release_capture(&self, node: NodeId) {
            release_capture(&self.cells, node);
        }
    }

    fn button_payload(button: u8) -> EventPayload {
        EventPayload::Mouse(MousePayload {
            button,
            ..MousePayload::at(1.0, 1.0, inca_document::Modifiers::none())
        })
    }
}

/// Trees, payloads, windows and fake-script scenes.
pub(crate) mod native {
    use super::*;
    pub(crate) use crate::test_support::apply_focus;
    pub(crate) use crate::test_support::{
        Frame, add_sheet, at, draw, is_focused, mouse_at, node_under_root, state, tree_of_three,
        wheel_at,
    };

    pub(crate) fn set_active(
        cx: &mut TestAppContext,
        window: gpui::WindowHandle<Frame>,
        dispatcher: &EventDispatcher,
        active: bool,
    ) {
        cx.update_window(window.into(), |_, window, cx| {
            dispatcher.window_activated(active, window, cx);
        })
        .unwrap();
    }

    /// A focusable node under the root of `cells`, on which no script listens.
    pub(crate) fn fake_focusable_node(cells: &Rc<Cells>) -> NodeId {
        let mut document = cells.document.borrow_mut();
        let node = document.tree.create_node("div").unwrap();
        let root = document.root();
        document.tree.append_child(root, node).unwrap();
        document
            .tree
            .set_attribute(node, "tabindex", inca_node::AttributeValue::from("0"))
            .unwrap();
        document.focus.mark_tab_dirty(node);
        node
    }

    /// A window over one focusable node, on a [`FakeScript`].
    pub(crate) fn fake_focus_scene(
        cx: &mut TestAppContext,
    ) -> (
        Rc<EventDispatcher>,
        Rc<Cells>,
        Rc<FakeScript>,
        gpui::WindowHandle<Frame>,
        NodeId,
    ) {
        let (dispatcher, cells, fake) = fake_dispatcher();
        let node = fake_focusable_node(&cells);
        let dispatcher = Rc::new(dispatcher);
        let window = cx.add_window(|_, _| Frame(Rc::clone(&dispatcher)));
        (dispatcher, cells, fake, window, node)
    }
}

pub(crate) use native::*;
