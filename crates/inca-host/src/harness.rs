// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Drives a mounted app headlessly: reads its tree, sends input and waits
//! for it to go idle.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    KeyUpEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point,
    ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, WindowHandle, point, px,
};
use inca_bridge::Host;
use inca_gpui::{NodeId, debug_selector};
use inca_jsenv::stderr_reporter;

use crate::app::{HostedApp, start};
use crate::snapshot::{Bounds, Node, snapshot};

/// An app mounted in a test window.
///
/// The window, config, menu and sizing are the ones `run_bundle` sets up.
/// Every input method settles the app before it returns, so a following
/// [`Harness::snapshot`] sees the result.
pub struct Harness {
    cx: VisualTestContext,
    window: WindowHandle<HostedApp>,
    host: Rc<RefCell<Host>>,
    // Each selector string is leaked once per node id, because
    // `debug_bounds` takes `&'static str`.
    selectors: HashMap<NodeId, &'static str>,
}

impl Harness {
    /// Mounts the bundle `source`, read from `entry_path`, in a window of
    /// `cx` and settles it.
    ///
    /// # Panics
    ///
    /// Panics when the bundle fails to start, for example by throwing, or when
    /// the window cannot open.
    pub fn load(cx: &mut TestAppContext, entry_path: &str, source: &str) -> Self {
        let window = cx
            .update(|cx| start(cx, entry_path, source, stderr_reporter()))
            .unwrap_or_else(|failure| panic!("the bundle failed to start: {failure:?}"));
        let host = cx
            .update(|cx| window.read_with(cx, |app, _| Rc::clone(&app.session.host)))
            .expect("the window just opened");
        let mut harness = Self {
            cx: VisualTestContext::from_window(window.into(), cx),
            window,
            host,
            selectors: HashMap::new(),
        };
        harness.settle();
        harness
    }

    /// The node tree as last drawn.
    ///
    /// # Panics
    ///
    /// Panics when the app has no root node.
    pub fn snapshot(&mut self) -> Node {
        let host = Rc::clone(&self.host);
        let host = host.borrow();
        snapshot(&host.tree, host.root, &mut |id| self.bounds_of(id)).expect("the root node exists")
    }

    /// Clicks the center of node `id`'s painted bounds. A node without
    /// bounds, such as a text node or one that was not drawn, is clicked at
    /// its nearest ancestor with bounds. Content scrolled outside its
    /// container's viewport is clicked at that off-screen point. A zero-size
    /// node is clicked at its own point.
    ///
    /// # Panics
    ///
    /// Panics when no node has id `id`, or when neither it nor any ancestor
    /// has bounds.
    pub fn click(&mut self, id: NodeId) {
        let at = self.center_of(id);
        self.cx.simulate_click(at, Modifiers::none());
        self.settle();
    }

    /// Presses and releases `button` at the center of node `id`, located as
    /// in [`Harness::click`], with `modifiers` held and `click_count` as the
    /// number of clicks the press is part of.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Harness::click`].
    pub fn click_with(
        &mut self,
        id: NodeId,
        button: MouseButton,
        modifiers: Modifiers,
        click_count: usize,
    ) {
        let position = self.center_of(id);
        self.move_pointer(position);
        self.cx.simulate_event(MouseDownEvent {
            position,
            button,
            modifiers,
            click_count,
            first_mouse: false,
        });
        self.cx.simulate_event(MouseUpEvent {
            position,
            button,
            modifiers,
            click_count,
        });
        self.settle();
    }

    /// Moves the pointer to the center of node `id`, located as in
    /// [`Harness::click`]. The node and its ancestors receive `mouseenter`
    /// when the pointer was outside them. Call [`Harness::unhover`] to leave.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Harness::click`].
    pub fn hover(&mut self, id: NodeId) {
        let at = self.center_of(id);
        self.move_pointer(at);
    }

    /// Moves the pointer to a point outside every node, so each node it was
    /// over receives `mouseleave`.
    pub fn unhover(&mut self) {
        self.move_pointer(point(px(-1.0), px(-1.0)));
    }

    /// Scrolls with a pixel wheel event of (`dx`, `dy`) over node `id`,
    /// located as in [`Harness::click`]. A negative `dy` scrolls down.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Harness::click`].
    pub fn scroll(&mut self, id: NodeId, dx: f32, dy: f32) {
        let position = self.center_of(id);
        self.cx.simulate_event(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
            ..Default::default()
        });
        self.settle();
    }

    /// Sends space-separated keystrokes, such as `"a cmd-b enter"`, to the
    /// focused node. Each key is pressed (key-down only);
    /// [`Harness::key_up`] releases it.
    ///
    /// # Panics
    ///
    /// Panics when a keystroke does not parse.
    pub fn keystrokes(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.settle();
    }

    /// Releases space-separated keys, such as `"a enter"`, to the focused
    /// node.
    ///
    /// # Panics
    ///
    /// Panics when a keystroke does not parse.
    pub fn key_up(&mut self, keys: &str) {
        for keystroke in keys.split(' ').map(Keystroke::parse) {
            self.cx.simulate_event(KeyUpEvent {
                keystroke: keystroke.expect("a parseable keystroke"),
            });
        }
        self.settle();
    }

    /// Runs one pass: the app's pending JS jobs, then queued work until none
    /// is left, then a window draw, so [`Harness::snapshot`] reads current
    /// bounds. Jobs scheduled by that draw run on the next call.
    ///
    /// # Panics
    ///
    /// Panics when the window has closed.
    pub fn settle(&mut self) {
        self.window
            .update(&mut self.cx, |app, window, cx| {
                app.session.dispatcher.drain_jobs_and_refresh(window, cx);
            })
            .expect("the window is open");
        self.cx.run_until_parked();
        self.cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    fn move_pointer(&mut self, at: Point<Pixels>) {
        self.cx.simulate_mouse_move(at, None, Modifiers::none());
        self.settle();
    }

    fn bounds_of(&mut self, id: NodeId) -> Option<Bounds> {
        let selector = *self
            .selectors
            .entry(id)
            .or_insert_with(|| Box::leak(debug_selector(id).into_boxed_str()));
        self.cx.debug_bounds(selector).map(|b| Bounds {
            x: f32::from(b.origin.x),
            y: f32::from(b.origin.y),
            width: f32::from(b.size.width),
            height: f32::from(b.size.height),
        })
    }

    fn center_of(&mut self, id: NodeId) -> Point<Pixels> {
        let root = self.snapshot();
        let bounds = match nearest_bounds(&root, id, None) {
            Lookup::At(bounds) => bounds,
            Lookup::Absent => panic!("no node {id} in the tree"),
            Lookup::Unbounded => panic!("node {id} and its ancestors have no bounds"),
        };
        point(
            px(bounds.x + bounds.width / 2.0),
            px(bounds.y + bounds.height / 2.0),
        )
    }
}

/// Where [`nearest_bounds`] ended up.
#[derive(Debug, PartialEq)]
enum Lookup {
    /// `id` is not under the node.
    Absent,
    /// `id` is there, and neither it nor an ancestor has bounds.
    Unbounded,
    At(Bounds),
}

/// The bounds of node `id` or of its nearest ancestor that has any.
fn nearest_bounds(node: &Node, id: NodeId, inherited: Option<Bounds>) -> Lookup {
    let here = node.bounds.or(inherited);
    if node.id == id {
        return here.map_or(Lookup::Unbounded, Lookup::At);
    }
    node.children
        .iter()
        .map(|child| nearest_bounds(child, id, here))
        .find(|found| *found != Lookup::Absent)
        .unwrap_or(Lookup::Absent)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests;
