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
use inca_bridge::{Host, stderr_reporter};
use inca_gpui::{NodeId, debug_selector};

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
            .update(|cx| start(cx, entry_path, source, stderr_reporter(), None))
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
            .update(&mut self.cx, |app, window, _| {
                app.session.dispatcher.drain_jobs_and_refresh(window);
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
mod tests {
    use super::*;
    use inca_gpui::EventSink;
    use std::collections::BTreeMap;

    const ENTRY: &str = "/test/entry.js";

    /// A 100x50 box holding a text leaf, with a click listener that counts.
    const CLICKABLE: &str = r"
        const n = __inca_native__;
        const box = n.createNode('div');
        n.setStyle(box, 'width', 100);
        n.setStyle(box, 'height', 50);
        n.appendChild(n.rootNodeId(), box);
        const label = n.createNode('text');
        n.setAttribute(label, 'value', 'hi');
        n.appendChild(box, label);
        globalThis.clicks = 0;
        globalThis.__inca_callbacks__ = { 0: () => { globalThis.clicks++; } };
        n.addEventListener(box, 'click', 0);
    ";

    fn eval_f64(h: &Harness, code: &str) -> f64 {
        h.window
            .read_with(&h.cx, |app, _| {
                app.session.engine.eval::<f64>(code).unwrap()
            })
            .unwrap()
    }

    fn id_where(h: &mut Harness, f: impl Fn(&Node) -> bool) -> NodeId {
        h.snapshot().find(&f).unwrap().id
    }

    #[gpui::test]
    fn load_mounts_the_bundle_and_reports_bounds(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, CLICKABLE);

        let snap = h.snapshot();
        let boxed = snap
            .find(&|n| n.children.iter().any(|c| c.text.is_some()))
            .unwrap();
        let b = boxed.bounds.unwrap();
        assert_eq!((b.width, b.height), (100.0, 50.0));
        assert_eq!(boxed.children[0].text.as_deref(), Some("hi"));
        assert_eq!(boxed.children[0].bounds, None);
    }

    #[gpui::test]
    #[should_panic(expected = "the bundle failed to start")]
    fn load_panics_when_the_bundle_throws(cx: &mut TestAppContext) {
        Harness::load(cx, ENTRY, "throw new Error('boom');");
    }

    #[gpui::test]
    fn click_reaches_a_listener_on_the_node(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, CLICKABLE);
        let boxed = id_where(&mut h, |n| {
            n.tag == "div" && n.bounds.is_some_and(|b| b.width == 100.0)
        });

        h.click(boxed);

        assert_eq!(eval_f64(&h, "globalThis.clicks"), 1.0);
    }

    #[gpui::test]
    fn click_on_a_text_leaf_lands_on_its_container(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, CLICKABLE);
        let leaf = id_where(&mut h, |n| n.text.is_some());

        h.click(leaf);

        assert_eq!(eval_f64(&h, "globalThis.clicks"), 1.0);
    }

    #[gpui::test]
    #[should_panic(expected = "no node 9999 in the tree")]
    fn click_panics_on_an_unknown_node(cx: &mut TestAppContext) {
        Harness::load(cx, ENTRY, CLICKABLE).click(9999);
    }

    /// A 100x50 box below a spacer, counting its `mouseenter` and `mouseleave`
    /// events. The pointer starts over the spacer.
    const HOVERABLE: &str = r"
        const n = __inca_native__;
        const frame = n.createNode('div');
        n.setStyle(frame, 'width', 200);
        n.setStyle(frame, 'height', 200);
        n.appendChild(n.rootNodeId(), frame);
        const spacer = n.createNode('div');
        n.setStyle(spacer, 'width', 60);
        n.setStyle(spacer, 'height', 50);
        n.appendChild(frame, spacer);
        const box = n.createNode('div');
        n.setStyle(box, 'width', 100);
        n.setStyle(box, 'height', 50);
        n.appendChild(frame, box);
        globalThis.enters = 0;
        globalThis.leaves = 0;
        globalThis.__inca_callbacks__ = {
            0: () => { globalThis.enters++; },
            1: () => { globalThis.leaves++; },
        };
        n.addEventListener(box, 'mouseenter', 0);
        n.addEventListener(box, 'mouseleave', 1);
    ";

    fn hover_box(h: &mut Harness) -> NodeId {
        id_where(h, |n| n.bounds.is_some_and(|b| b.width == 100.0))
    }

    #[gpui::test]
    fn hover_fires_mouseenter_once(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, HOVERABLE);
        let boxed = hover_box(&mut h);
        assert_eq!(eval_f64(&h, "globalThis.enters"), 0.0);

        h.hover(boxed);

        assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
        assert_eq!(eval_f64(&h, "globalThis.leaves"), 0.0);
    }

    #[gpui::test]
    fn hovering_the_same_node_again_does_not_repeat_mouseenter(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, HOVERABLE);
        let boxed = hover_box(&mut h);

        h.hover(boxed);
        h.hover(boxed);

        assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
    }

    #[gpui::test]
    fn unhover_fires_mouseleave(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, HOVERABLE);
        let boxed = hover_box(&mut h);
        h.hover(boxed);

        h.unhover();

        assert_eq!(eval_f64(&h, "globalThis.leaves"), 1.0);
    }

    #[gpui::test]
    fn hover_again_after_leaving_fires_mouseenter_again(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, HOVERABLE);
        let boxed = hover_box(&mut h);

        h.hover(boxed);
        h.unhover();
        h.hover(boxed);

        assert_eq!(eval_f64(&h, "globalThis.enters"), 2.0);
        assert_eq!(eval_f64(&h, "globalThis.leaves"), 1.0);
    }

    #[gpui::test]
    fn unhover_twice_fires_mouseleave_once(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, HOVERABLE);
        let boxed = hover_box(&mut h);
        h.hover(boxed);

        h.unhover();
        h.unhover();

        assert_eq!(eval_f64(&h, "globalThis.leaves"), 1.0);
    }

    #[gpui::test]
    fn unhover_before_any_hover_fires_nothing(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            ENTRY,
            r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.setStyle(box, 'margin_left', 200);
            n.appendChild(n.rootNodeId(), box);
            globalThis.leaves = 0;
            globalThis.__inca_callbacks__ = { 0: () => { globalThis.leaves++; } };
            n.addEventListener(box, 'mouseleave', 0);
            ",
        );

        h.unhover();

        assert_eq!(eval_f64(&h, "globalThis.leaves"), 0.0);
    }

    #[gpui::test]
    fn hovering_a_child_enters_its_ancestors(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            ENTRY,
            r"
            const n = __inca_native__;
            const frame = n.createNode('div');
            n.setStyle(frame, 'width', 200);
            n.setStyle(frame, 'height', 200);
            n.appendChild(n.rootNodeId(), frame);
            const spacer = n.createNode('div');
            n.setStyle(spacer, 'width', 100);
            n.setStyle(spacer, 'height', 50);
            n.appendChild(frame, spacer);
            const outer = n.createNode('div');
            n.setStyle(outer, 'width', 100);
            n.setStyle(outer, 'height', 100);
            n.appendChild(frame, outer);
            const inner = n.createNode('div');
            n.setStyle(inner, 'width', 20);
            n.setStyle(inner, 'height', 20);
            n.appendChild(outer, inner);
            globalThis.enters = 0;
            globalThis.__inca_callbacks__ = { 0: () => { globalThis.enters++; } };
            n.addEventListener(outer, 'mouseenter', 0);
            ",
        );
        assert_eq!(eval_f64(&h, "globalThis.enters"), 0.0);
        let inner = id_where(&mut h, |n| n.bounds.is_some_and(|b| b.width == 20.0));

        h.hover(inner);

        assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
    }

    #[gpui::test]
    fn hover_on_a_text_leaf_lands_on_its_container(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            ENTRY,
            &format!(
                "{HOVERABLE}
                const label = n.createNode('text');
                n.setAttribute(label, 'value', 'hi');
                n.appendChild(box, label);"
            ),
        );
        let leaf = id_where(&mut h, |n| n.text.is_some());

        h.hover(leaf);

        assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
    }

    #[gpui::test]
    #[should_panic(expected = "no node 9999 in the tree")]
    fn hover_panics_on_an_unknown_node(cx: &mut TestAppContext) {
        Harness::load(cx, ENTRY, HOVERABLE).hover(9999);
    }

    #[test]
    fn nearest_bounds_inherits_from_the_closest_ancestor() {
        let b = |x| Bounds {
            x,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        let node = |id, bounds, children| Node {
            id,
            tag: "div".into(),
            text: None,
            attributes: BTreeMap::new(),
            bounds,
            children,
        };
        let tree = node(
            1,
            Some(b(1.0)),
            vec![node(2, Some(b(2.0)), vec![node(3, None, vec![])])],
        );

        assert_eq!(nearest_bounds(&tree, 3, None), Lookup::At(b(2.0)));
        assert_eq!(nearest_bounds(&tree, 1, None), Lookup::At(b(1.0)));
        assert_eq!(nearest_bounds(&tree, 9, None), Lookup::Absent);
        let bare = node(1, None, vec![node(2, None, vec![])]);
        assert_eq!(nearest_bounds(&bare, 2, None), Lookup::Unbounded);
    }

    #[gpui::test]
    fn a_zero_size_node_resolves_to_its_own_point(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            ENTRY,
            r"
            const n = __inca_native__;
            const z = n.createNode('div');
            n.setStyle(z, 'width', 0);
            n.setStyle(z, 'height', 0);
            n.appendChild(n.rootNodeId(), z);
            ",
        );
        let snap = h.snapshot();
        let z = snap.children[0].clone();
        let b = z.bounds.unwrap();
        assert_eq!((b.width, b.height), (0.0, 0.0));

        assert_eq!(h.center_of(z.id), point(px(b.x), px(b.y)));
    }

    #[gpui::test]
    fn scroll_moves_the_container_offset(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            ENTRY,
            r"
            const n = __inca_native__;
            const c = n.createNode('div');
            n.setStyle(c, 'width', 100);
            n.setStyle(c, 'height', 50);
            n.setStyle(c, 'overflow_y', 'scroll');
            n.appendChild(n.rootNodeId(), c);
            const inner = n.createNode('div');
            n.setStyle(inner, 'width', 100);
            n.setStyle(inner, 'height', 300);
            n.appendChild(c, inner);
            ",
        );
        let container = id_where(&mut h, |n| {
            n.children
                .iter()
                .any(|c| c.bounds.is_some_and(|b| b.height == 300.0))
        });

        h.scroll(container, 0.0, -30.0);

        let offset = h
            .window
            .read_with(&h.cx, |app, _| {
                app.session
                    .dispatcher
                    .scroll_handle(container)
                    .unwrap()
                    .offset()
            })
            .unwrap();
        assert_eq!(offset.y, px(-30.0));
    }

    #[gpui::test]
    fn keystrokes_reach_the_focused_node(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            ENTRY,
            r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.appendChild(n.rootNodeId(), box);
            globalThis.keys = 0;
            globalThis.__inca_callbacks__ = { 0: () => { globalThis.keys++; } };
            n.addEventListener(box, 'keydown', 0);
            n.setAttribute(box, 'tabindex', '0');
            n.focusNode(box);
            ",
        );

        h.keystrokes("a b");

        assert_eq!(eval_f64(&h, "globalThis.keys"), 2.0);
    }

    #[gpui::test]
    fn key_up_reaches_the_focused_node_once_per_key(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            ENTRY,
            r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.appendChild(n.rootNodeId(), box);
            globalThis.ups = 0;
            globalThis.downs = 0;
            globalThis.__inca_callbacks__ = {
                0: () => { globalThis.ups++; },
                1: () => { globalThis.downs++; },
            };
            n.addEventListener(box, 'keyup', 0);
            n.addEventListener(box, 'keydown', 1);
            n.setAttribute(box, 'tabindex', '0');
            n.focusNode(box);
            ",
        );

        h.key_up("a b");

        assert_eq!(eval_f64(&h, "globalThis.ups"), 2.0);
        assert_eq!(eval_f64(&h, "globalThis.downs"), 0.0);
    }

    #[gpui::test]
    fn settle_flushes_a_microtask_mount(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, "");
        let before = h.snapshot().children.len();
        h.window
            .read_with(&h.cx, |app, _| {
                app.session
                    .engine
                    .eval::<()>(
                        r"Promise.resolve().then(() => {
                            const n = __inca_native__;
                            const d = n.createNode('div');
                            n.setStyle(d, 'width', 40);
                            n.setStyle(d, 'height', 20);
                            n.appendChild(n.rootNodeId(), d);
                        });",
                    )
                    .unwrap();
            })
            .unwrap();
        assert_eq!(h.snapshot().children.len(), before);

        h.settle();

        let snap = h.snapshot();
        assert_eq!(snap.children.len(), before + 1);
        assert_eq!(snap.children[before].bounds.unwrap().width, 40.0);
    }

    const SCROLLER: &str = r"
        const n = __inca_native__;
        const c = n.createNode('div');
        n.setStyle(c, 'width', 100);
        n.setStyle(c, 'height', 50);
        n.setStyle(c, 'overflow_y', 'scroll');
        n.setStyle(c, 'overflow_x', 'scroll');
        n.appendChild(n.rootNodeId(), c);
        const label = n.createNode('text');
        n.setAttribute(label, 'value', 'hi');
        n.appendChild(c, label);
        const inner = n.createNode('div');
        n.setStyle(inner, 'width', 300);
        n.setStyle(inner, 'height', 300);
        n.appendChild(c, inner);
    ";

    fn offset(h: &Harness, id: NodeId) -> Point<Pixels> {
        h.window
            .read_with(&h.cx, |app, _| {
                app.session.dispatcher.scroll_handle(id).unwrap().offset()
            })
            .unwrap()
    }

    #[gpui::test]
    fn scroll_on_a_text_leaf_scrolls_its_container(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, SCROLLER);
        let container = id_where(&mut h, |n| n.children.iter().any(|c| c.text.is_some()));
        let leaf = id_where(&mut h, |n| n.text.is_some());

        h.scroll(leaf, 0.0, -20.0);

        assert_eq!(offset(&h, container).y, px(-20.0));
    }

    #[gpui::test]
    fn scroll_moves_the_horizontal_offset(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, SCROLLER);
        let container = id_where(&mut h, |n| n.children.iter().any(|c| c.text.is_some()));

        h.scroll(container, -25.0, 0.0);

        assert_eq!(offset(&h, container).x, px(-25.0));
    }

    #[gpui::test]
    fn click_aims_at_content_scrolled_out_of_its_viewport(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            ENTRY,
            r"
            const n = __inca_native__;
            const c = n.createNode('div');
            n.setStyle(c, 'width', 100);
            n.setStyle(c, 'height', 50);
            n.setStyle(c, 'overflow_y', 'scroll');
            n.appendChild(n.rootNodeId(), c);
            const spacer = n.createNode('div');
            n.setStyle(spacer, 'width', 100);
            n.setStyle(spacer, 'height', 200);
            n.appendChild(c, spacer);
            const target = n.createNode('div');
            n.setStyle(target, 'width', 100);
            n.setStyle(target, 'height', 40);
            n.appendChild(c, target);
            ",
        );
        let container = id_where(&mut h, |n| {
            n.children
                .iter()
                .any(|c| c.bounds.is_some_and(|b| b.height == 40.0))
        });
        let target = id_where(&mut h, |n| n.bounds.is_some_and(|b| b.height == 40.0));
        h.scroll(container, 0.0, -100.0);

        let at = h.center_of(target);

        let view = h
            .snapshot()
            .find(&|n| n.id == container)
            .unwrap()
            .bounds
            .unwrap();
        assert!(f32::from(at.y) > view.y + view.height);
    }

    #[gpui::test]
    fn click_on_a_node_that_was_not_drawn_uses_its_ancestor(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            ENTRY,
            r"
            const n = __inca_native__;
            const hidden = n.createNode('div');
            n.setStyle(hidden, 'width', 60);
            n.setStyle(hidden, 'height', 30);
            n.setStyle(hidden, 'display', 'none');
            n.appendChild(n.rootNodeId(), hidden);
            const inner = n.createNode('div');
            n.setStyle(inner, 'width', 10);
            n.setStyle(inner, 'height', 10);
            n.appendChild(hidden, inner);
            ",
        );
        let snap = h.snapshot();
        let inner = snap.children[0].children[0].clone();
        assert_eq!(inner.bounds, None);
        let ancestor = snap.children[0].bounds.unwrap();

        let at = h.center_of(inner.id);

        assert_eq!(
            at,
            point(
                px(ancestor.x + ancestor.width / 2.0),
                px(ancestor.y + ancestor.height / 2.0)
            )
        );
    }

    #[gpui::test]
    #[should_panic(expected = "InvalidKeystrokeError")]
    fn keystrokes_panic_on_an_unparseable_key(cx: &mut TestAppContext) {
        Harness::load(cx, ENTRY, "").keystrokes("a-b");
    }

    /// A 200x200 frame holding a 100x50 `box` at its top-left corner. `box`
    /// records its `focus` and `blur` events in `globalThis.log`. The frame's
    /// centre lies outside `box`.
    const FOCUSABLE: &str = r"
        const n = __inca_native__;
        const frame = n.createNode('div');
        n.setStyle(frame, 'width', 200);
        n.setStyle(frame, 'height', 200);
        n.appendChild(n.rootNodeId(), frame);
        const box = n.createNode('div');
        n.setStyle(box, 'width', 100);
        n.setStyle(box, 'height', 50);
        n.appendChild(frame, box);
        globalThis.frame = frame;
        globalThis.box = box;
        globalThis.log = [];
        globalThis.__inca_callbacks__ = {
            0: (e) => { globalThis.log.push(e.type); },
            1: (e) => { e.preventDefault(); },
        };
        n.addEventListener(box, 'focus', 0);
        n.addEventListener(box, 'blur', 0);
    ";

    /// Makes `box` focusable and focuses it.
    const FOCUS_BOX: &str =
        "__inca_native__.setAttribute(box, 'tabindex', '0'); __inca_native__.focusNode(box);";

    fn run_js(h: &mut Harness, code: &str) {
        h.window
            .read_with(&h.cx, |app, _| app.session.engine.eval::<()>(code).unwrap())
            .unwrap();
        h.settle();
    }

    fn log(h: &Harness) -> String {
        h.window
            .read_with(&h.cx, |app, _| {
                app.session
                    .engine
                    .eval::<String>("globalThis.log.join(',')")
                    .unwrap()
            })
            .unwrap()
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn node(h: &Harness, name: &str) -> NodeId {
        eval_f64(h, &format!("globalThis.{name}")) as NodeId
    }

    #[gpui::test]
    fn a_click_focuses_a_tabindex_node(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, FOCUSABLE);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(box, 'tabindex', '0');",
        );
        let boxed = node(&h, "box");
        h.click(boxed);
        assert_eq!(log(&h), "focus");
    }

    #[gpui::test]
    fn default_prevented_carries_over_to_the_ancestors_listener(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, FOCUSABLE);
        run_js(
            &mut h,
            "__inca_callbacks__[2] = (e) => { const was = e.defaultPrevented; e.preventDefault(); \
                globalThis.log.push([e.type, was, e.defaultPrevented].join('/')); }; \
             __inca_native__.addEventListener(box, 'click', 2); \
             __inca_native__.addEventListener(frame, 'click', 2);",
        );
        let boxed = node(&h, "box");
        h.click(boxed);
        assert_eq!(log(&h), "click/false/true,click/true/true");
    }

    #[gpui::test]
    fn focusin_and_focusout_bubble(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, FOCUSABLE);
        run_js(
            &mut h,
            "__inca_callbacks__[2] = (e) => { globalThis.log.push([e.type, \
                e.target === box ? 'box' : 'other', e.eventPhase].join('/')); }; \
             for (const t of [box, frame]) { \
                __inca_native__.addEventListener(t, 'focusin', 2); \
                __inca_native__.addEventListener(t, 'focusout', 2); } \
             __inca_native__.setAttribute(box, 'tabindex', '0'); __inca_native__.focusNode(box);",
        );
        let frame = node(&h, "frame");
        h.click(frame);
        assert_eq!(
            log(&h),
            "focus,focusin/box/2,focusin/box/3,\
             blur,focusout/box/2,focusout/box/3"
        );
    }

    #[gpui::test]
    fn a_negative_tabindex_takes_click_and_focus(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, FOCUSABLE);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(box, 'tabindex', '-1');",
        );
        let boxed = node(&h, "box");
        h.click(boxed);
        assert_eq!(log(&h), "focus");
        let frame = node(&h, "frame");
        h.click(frame);
        run_js(&mut h, "__inca_native__.focusNode(box);");
        assert_eq!(log(&h), "focus,blur,focus");
    }

    #[gpui::test]
    fn a_click_on_an_empty_area_blurs_the_focused_node(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, FOCUSABLE);
        run_js(&mut h, FOCUS_BOX);
        let frame = node(&h, "frame");
        h.click(frame);
        assert_eq!(log(&h), "focus,blur");
    }

    #[gpui::test]
    fn a_prevented_mousedown_keeps_the_focused_node(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, FOCUSABLE);
        run_js(
            &mut h,
            &format!("{FOCUS_BOX} __inca_native__.addEventListener(frame, 'mousedown', 1);"),
        );
        let frame = node(&h, "frame");
        h.click(frame);
        assert_eq!(log(&h), "focus");
    }

    #[gpui::test]
    fn a_prevented_mousedown_on_the_root_keeps_the_focused_node(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, FOCUSABLE);
        run_js(
            &mut h,
            &format!(
                "{FOCUS_BOX} __inca_native__.addEventListener(__inca_native__.rootNodeId(), 'mousedown', 1);"
            ),
        );
        let frame = node(&h, "frame");
        h.click(frame);
        assert_eq!(log(&h), "focus");
    }

    #[gpui::test]
    fn hiding_a_focused_node_blurs_it(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, FOCUSABLE);
        run_js(&mut h, FOCUS_BOX);
        run_js(&mut h, "__inca_native__.setStyle(box, 'display', 'none');");
        assert_eq!(log(&h), "focus,blur");
    }

    #[gpui::test]
    fn a_prevented_mousedown_does_not_focus_the_node(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, FOCUSABLE);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(box, 'tabindex', '0'); \
             __inca_native__.addEventListener(box, 'mousedown', 1);",
        );
        let boxed = node(&h, "box");
        h.click(boxed);
        assert_eq!(log(&h), "");
    }

    /// A 200x200 `frame` holding a 100x50 `button` named `box` at its
    /// top-left corner, with `box` focused. Callback 0 logs each event as
    /// `type@currentTarget/target`, 1 calls `preventDefault`, 2 calls
    /// `stopPropagation` and 3 logs the click fields.
    const BUTTON: &str = r"
        const n = __inca_native__;
        const frame = n.createNode('div');
        n.setStyle(frame, 'width', 200);
        n.setStyle(frame, 'height', 200);
        n.appendChild(n.rootNodeId(), frame);
        const box = n.createNode('button');
        n.setStyle(box, 'width', 100);
        n.setStyle(box, 'height', 50);
        n.appendChild(frame, box);
        globalThis.frame = frame;
        globalThis.box = box;
        globalThis.log = [];
        const name = (id) => (id === box ? 'box' : id === frame ? 'frame' : id === n.rootNodeId() ? 'root' : 'kid');
        globalThis.__inca_callbacks__ = {
            0: (e) => { globalThis.log.push(`${e.type}@${name(e.currentTarget)}/${name(e.target)}`); },
            1: (e) => { e.preventDefault(); },
            2: (e) => { e.stopPropagation(); },
            3: (e) => {
                globalThis.log.push([e.detail, e.button, e.buttons, e.clientX, e.clientY, e.shiftKey,
                    e.pointerId, e.pointerType, e.isPrimary, e.width, e.height, e.pressure].join(':'));
            },
        };
        n.focusNode(box);
    ";

    /// Registers callback `callback` for each of `types` on `target`.
    fn listen(h: &mut Harness, target: &str, types: &str, callback: u32) {
        run_js(
            h,
            &format!(
                "for (const t of '{types}'.split(' ')) \
                 __inca_native__.addEventListener({target}, t, {callback});"
            ),
        );
    }

    #[gpui::test]
    fn enter_clicks_a_focused_button_on_key_down_each_time(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "keydown keyup click", 0);

        h.keystrokes("enter");
        assert_eq!(log(&h), "keydown@box/box,click@box/box");
        h.keystrokes("enter");
        h.key_up("enter");

        assert_eq!(
            log(&h),
            "keydown@box/box,click@box/box,keydown@box/box,click@box/box,keyup@box/box"
        );
    }

    #[gpui::test]
    fn space_clicks_a_focused_button_once_after_key_up(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "keydown keyup click", 0);

        h.keystrokes("space");
        h.keystrokes("space");
        assert_eq!(log(&h), "keydown@box/box,keydown@box/box");
        h.key_up("space");
        h.key_up("space");

        assert_eq!(
            log(&h),
            "keydown@box/box,keydown@box/box,keyup@box/box,click@box/box,keyup@box/box"
        );
    }

    #[gpui::test]
    fn a_space_key_up_clicks_only_after_its_key_down(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "click", 0);

        h.key_up("space");
        assert_eq!(log(&h), "");

        h.keystrokes("space");
        h.key_up("space");
        assert_eq!(log(&h), "click@box/box");
    }

    #[gpui::test]
    fn enter_clicks_with_shift_and_not_with_control_alt_or_meta(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "click", 3);

        h.keystrokes("ctrl-enter alt-enter cmd-enter");
        assert_eq!(log(&h), "");
        h.keystrokes("enter shift-enter");

        assert_eq!(
            log(&h),
            "0:0:0:0:0:false:-1::false:1:1:0,0:0:0:0:0:true:-1::false:1:1:0"
        );
    }

    #[gpui::test]
    fn a_pending_space_press_ends_at_the_next_key_press_or_mouse_press(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "click", 0);

        h.keystrokes("space a");
        h.key_up("space");
        assert_eq!(log(&h), "");

        h.keystrokes("space");
        let frame = node(&h, "frame");
        h.click(frame);
        run_js(&mut h, "__inca_native__.focusNode(box);");
        h.key_up("space");
        assert_eq!(log(&h), "");

        h.keystrokes("space");
        h.key_up("space");
        assert_eq!(log(&h), "click@box/box");
    }

    #[gpui::test]
    fn a_pending_space_press_does_not_click_another_button(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        run_js(
            &mut h,
            "globalThis.b2 = __inca_native__.createNode('button'); \
             __inca_native__.appendChild(frame, b2);",
        );
        listen(&mut h, "box", "click", 0);
        listen(&mut h, "b2", "click", 0);

        h.keystrokes("space");
        run_js(&mut h, "__inca_native__.focusNode(b2);");
        h.key_up("space");

        assert_eq!(log(&h), "");
    }

    #[gpui::test]
    fn space_clicks_on_key_up_with_a_modifier_held(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "click", 0);

        h.keystrokes("space");
        h.key_up("ctrl-space");

        assert_eq!(log(&h), "click@box/box");
    }

    #[gpui::test]
    fn a_prevented_key_down_cancels_the_enter_click(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "keydown", 1);
        listen(&mut h, "box", "click", 0);

        h.keystrokes("enter");

        assert_eq!(log(&h), "");
    }

    #[gpui::test]
    fn a_prevented_key_down_cancels_the_space_click(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "keydown", 1);
        listen(&mut h, "box", "click", 0);

        h.keystrokes("space");
        h.key_up("space");

        assert_eq!(log(&h), "");
    }

    #[gpui::test]
    fn an_ancestors_prevented_key_down_cancels_the_key_click(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "frame", "keydown", 1);
        listen(&mut h, "box", "click", 0);

        h.keystrokes("enter space");
        h.key_up("space");

        assert_eq!(log(&h), "");
    }

    #[gpui::test]
    fn a_prevented_key_up_cancels_the_space_click(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "keyup", 1);
        listen(&mut h, "box", "click", 0);

        h.keystrokes("space");
        h.key_up("space");
        assert_eq!(log(&h), "");
        // The cancelled press leaves nothing pending.
        run_js(
            &mut h,
            "__inca_native__.removeEventListener(box, 'keyup', 1);",
        );
        h.key_up("space");

        assert_eq!(log(&h), "");
    }

    #[gpui::test]
    fn a_key_click_bubbles_with_the_button_as_target(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "click", 0);
        listen(&mut h, "frame", "click", 0);
        listen(&mut h, "__inca_native__.rootNodeId()", "click", 0);

        h.keystrokes("enter");

        assert_eq!(log(&h), "click@box/box,click@frame/box,click@root/box");
    }

    #[gpui::test]
    fn stopping_a_key_click_keeps_it_from_the_ancestors(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "click", 2);
        listen(&mut h, "frame", "click", 0);

        h.keystrokes("enter");

        assert_eq!(log(&h), "");
    }

    #[gpui::test]
    fn a_key_press_in_a_focused_descendant_does_not_click_the_button(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "click", 0);
        run_js(
            &mut h,
            "globalThis.k = __inca_native__.createNode('div'); \
             __inca_native__.setAttribute(k, 'tabindex', '0'); \
             __inca_native__.appendChild(box, k); \
             __inca_native__.focusNode(k);",
        );

        h.keystrokes("enter space");
        h.key_up("space");

        assert_eq!(log(&h), "");
    }

    #[gpui::test]
    fn a_press_on_a_disabled_button_moves_focus_to_its_tabindex_ancestor(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        run_js(
            &mut h,
            "globalThis.other = __inca_native__.createNode('div'); \
             __inca_native__.setAttribute(other, 'tabindex', '0'); \
             __inca_native__.setStyle(other, 'width', 10); \
             __inca_native__.setStyle(other, 'height', 10); \
             __inca_native__.appendChild(__inca_native__.rootNodeId(), other); \
             __inca_native__.setAttribute(frame, 'tabindex', '0'); \
             __inca_native__.setAttribute(box, 'disabled', true); \
             __inca_native__.focusNode(other);",
        );
        listen(&mut h, "other", "focus blur", 0);
        listen(&mut h, "frame", "focus blur", 0);
        let button = node(&h, "box");

        h.click(button);

        assert_eq!(log(&h), "blur@kid/kid,focus@frame/frame");
    }

    #[gpui::test]
    fn enter_and_space_click_only_a_button(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        run_js(
            &mut h,
            "const d = __inca_native__.createNode('div'); \
             __inca_native__.setAttribute(d, 'tabindex', '0'); \
             __inca_native__.appendChild(frame, d); \
             __inca_native__.addEventListener(d, 'click', 0); \
             __inca_native__.addEventListener(d, 'keydown', 0); \
             __inca_native__.focusNode(d);",
        );

        h.keystrokes("enter space");
        h.key_up("enter space");

        assert_eq!(log(&h), "keydown@kid/kid,keydown@kid/kid");
    }

    #[gpui::test]
    fn a_mouse_click_reaches_a_button_and_its_ancestor(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "click", 0);
        listen(&mut h, "frame", "click", 0);
        let button = node(&h, "box");

        h.click(button);

        assert_eq!(log(&h), "click@box/box,click@frame/box");
    }

    #[gpui::test]
    fn a_disabled_button_takes_no_focus_and_no_key_clicks(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "focus blur keydown click", 0);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(box, 'disabled', true);",
        );

        run_js(&mut h, "__inca_native__.focusNode(box);");
        h.keystrokes("enter space");
        h.key_up("space");

        assert_eq!(log(&h), "blur@box/box");
    }

    #[gpui::test]
    fn disabling_a_focused_button_blurs_it_and_enabling_restores_focus(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "focus blur click", 0);

        run_js(
            &mut h,
            "__inca_native__.setAttribute(box, 'disabled', true);",
        );
        run_js(&mut h, "__inca_native__.removeAttribute(box, 'disabled');");
        run_js(&mut h, "__inca_native__.focusNode(box);");
        h.keystrokes("enter");

        assert_eq!(log(&h), "blur@box/box,focus@box/box,click@box/box");
    }

    #[gpui::test]
    fn a_disabled_button_and_its_ancestors_get_no_press_or_click(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        for target in ["box", "frame", "__inca_native__.rootNodeId()"] {
            listen(&mut h, target, "mousedown mouseup click", 0);
        }
        run_js(
            &mut h,
            "__inca_native__.setAttribute(box, 'disabled', true);",
        );
        let button = node(&h, "box");

        h.click(button);
        assert_eq!(log(&h), "");
        run_js(&mut h, "__inca_native__.removeAttribute(box, 'disabled');");
        h.click(button);

        assert!(log(&h).contains("click@box/box"), "{}", log(&h));
    }

    #[gpui::test]
    fn a_key_click_on_an_enabled_button_reaches_the_ancestor_after_a_disabled_hover(
        cx: &mut TestAppContext,
    ) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        run_js(
            &mut h,
            "globalThis.b2 = __inca_native__.createNode('button'); \
             __inca_native__.setStyle(b2, 'width', 100); \
             __inca_native__.setStyle(b2, 'height', 50); \
             __inca_native__.appendChild(frame, b2); \
             __inca_native__.setAttribute(box, 'disabled', true);",
        );
        listen(&mut h, "frame", "click", 0);
        let button = node(&h, "box");
        h.hover(button);
        run_js(&mut h, "__inca_native__.focusNode(b2);");

        h.keystrokes("enter");

        assert_eq!(log(&h), "click@frame/kid");
    }

    #[gpui::test]
    fn a_press_on_a_disabled_button_released_on_an_ancestor_is_dropped(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        for target in ["box", "frame", "__inca_native__.rootNodeId()"] {
            listen(&mut h, target, "mouseup click", 0);
        }
        run_js(
            &mut h,
            "__inca_native__.setAttribute(box, 'disabled', true);",
        );
        h.cx.simulate_event(gpui::MouseDownEvent {
            position: point(px(10.0), px(10.0)),
            button: gpui::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        });
        h.cx.simulate_event(gpui::MouseUpEvent {
            position: point(px(150.0), px(150.0)),
            button: gpui::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        });
        h.settle();
        assert_eq!(log(&h), "");

        run_js(&mut h, "__inca_native__.removeAttribute(box, 'disabled');");
        let button = node(&h, "box");
        h.click(button);
        assert!(log(&h).contains("click@box/box"), "{}", log(&h));
    }

    #[gpui::test]
    fn a_disabled_button_keeps_hover_and_move_events(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        listen(&mut h, "box", "mouseenter mousemove", 0);
        listen(&mut h, "frame", "mousemove", 0);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(box, 'disabled', true);",
        );
        let button = node(&h, "box");

        h.hover(button);

        let log = log(&h);
        for want in [
            "mouseenter@box/box",
            "mousemove@box/box",
            "mousemove@frame/box",
        ] {
            assert!(log.contains(want), "{log}");
        }
    }

    #[gpui::test]
    fn a_disabled_button_keeps_a_descendant_listener(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        run_js(
            &mut h,
            "globalThis.k = __inca_native__.createNode('div'); \
             __inca_native__.setStyle(k, 'width', 100); \
             __inca_native__.setStyle(k, 'height', 50); \
             __inca_native__.appendChild(box, k); \
             __inca_native__.addEventListener(k, 'click', 0); \
             __inca_native__.addEventListener(box, 'click', 0); \
             __inca_native__.addEventListener(frame, 'click', 0); \
             __inca_native__.setAttribute(box, 'disabled', true);",
        );
        let kid = node(&h, "k");

        h.click(kid);

        assert_eq!(log(&h), "click@kid/kid");
    }

    #[gpui::test]
    #[should_panic(expected = "InvalidKeystrokeError")]
    fn key_up_panics_on_an_unparseable_key(cx: &mut TestAppContext) {
        Harness::load(cx, ENTRY, "").key_up("a-b");
    }

    #[gpui::test]
    #[should_panic(expected = "the window is open")]
    fn settle_panics_once_the_window_has_closed(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, "");
        h.cx.update(|window, _| window.remove_window());
        h.settle();
    }

    // frame > mid (scrolls) > [inner, label (text), tall]; `log` records
    // `type@currentTarget/target/eventPhase` by node name.
    const TARGETS: &str = r"
        const n = __inca_native__;
        const make = (parent, w, h) => {
            const id = n.createNode('div');
            n.setStyle(id, 'width', w);
            n.setStyle(id, 'height', h);
            n.appendChild(parent, id);
            return id;
        };
        const frame = make(n.rootNodeId(), 200, 200);
        const mid = make(frame, 100, 50);
        n.setStyle(mid, 'overflow_y', 'scroll');
        const inner = make(mid, 30, 30);
        const label = n.createNode('text');
        n.setAttribute(label, 'value', 'hi');
        n.appendChild(mid, label);
        const tall = make(mid, 100, 300);
        Object.assign(globalThis, { frame, mid, inner, label, tall });
        globalThis.log = [];
        const names = { [frame]: 'frame', [mid]: 'mid', [inner]: 'inner' };
        const name = (id) => names[id] ?? 'other';
        globalThis.__inca_callbacks__ = {
            0: (e) => { globalThis.log.push(
                `${e.type}@${name(e.currentTarget)}/${name(e.target)}/${e.eventPhase}`); },
        };
    ";

    fn targets_log(h: &mut Harness) -> String {
        let log = log(h);
        run_js(h, "globalThis.log.length = 0;");
        log
    }

    #[gpui::test]
    fn a_listener_on_an_ancestor_reads_the_deepest_node_as_target(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        listen(
            &mut h,
            "frame",
            "mousemove mousedown mouseup click wheel",
            0,
        );
        let inner = node(&h, "inner");

        h.hover(inner);
        h.click(inner);
        h.scroll(inner, 0.0, -5.0);

        // The host fires click before mouseup.
        assert_eq!(
            targets_log(&mut h),
            "mousemove@frame/inner/3,mousedown@frame/inner/3,click@frame/inner/3,\
             mouseup@frame/inner/3,wheel@frame/inner/3"
        );
    }

    #[gpui::test]
    fn a_text_child_reports_its_container_as_target(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        listen(&mut h, "frame", "click", 0);
        let label = node(&h, "label");

        h.click(label);

        assert_eq!(targets_log(&mut h), "click@frame/mid/3");
    }

    #[gpui::test]
    fn the_phase_is_2_where_current_target_is_target(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        listen(&mut h, "inner", "click", 0);
        listen(&mut h, "mid", "click", 0);
        listen(&mut h, "frame", "click", 0);
        let inner = node(&h, "inner");
        let mid = node(&h, "mid");

        h.click(inner);
        assert_eq!(
            targets_log(&mut h),
            "click@inner/inner/2,click@mid/inner/3,click@frame/inner/3"
        );
        h.click(mid);
        assert_eq!(targets_log(&mut h), "click@mid/mid/2,click@frame/mid/3");
    }

    #[gpui::test]
    fn a_key_event_targets_the_focused_node(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(inner, 'tabindex', '0'); __inca_native__.focusNode(inner);",
        );
        listen(&mut h, "frame", "keydown keyup focus blur", 0);
        listen(&mut h, "inner", "focus blur", 0);

        h.keystrokes("a");
        h.key_up("a");
        run_js(&mut h, "__inca_native__.blurNode(inner);");

        assert_eq!(
            targets_log(&mut h),
            "keydown@frame/inner/3,keyup@frame/inner/3,blur@inner/inner/2"
        );
    }

    #[gpui::test]
    fn an_unfocused_key_event_targets_the_root_and_reads_location_composing_and_modifiers(
        cx: &mut TestAppContext,
    ) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            r"
            const root = __inca_native__.rootNodeId();
            globalThis.__inca_callbacks__[1] = (e) => {
                globalThis.log.push([e.type, e.currentTarget === root,
                    e.target === root, e.eventPhase, e.key, e.location,
                    e.isComposing, e.repeat, e.getModifierState('Shift'),
                    e.getModifierState('Control'), e.getModifierState('Alt'),
                    e.getModifierState('Meta'), e.getModifierState('Accel'),
                    e.getModifierState('CapsLock')].join(':'));
            };
            __inca_native__.addEventListener(root, 'keydown', 1);
            __inca_native__.addEventListener(root, 'keyup', 1);
            __inca_native__.addEventListener(frame, 'keydown', 0);
            ",
        );

        h.keystrokes("shift-a");
        h.key_up("ctrl-b");

        assert_eq!(
            targets_log(&mut h),
            "keydown:true:true:2:A:0:false:false:true:false:false:false:false:false,\
             keyup:true:true:2:b:0:false:false:false:true:false:false:true:false"
        );
    }

    /// Callback 1 logs `type:key:` and the four modifier flags as 0 or 1.
    const MODIFIER_LOG: &str = "globalThis.__inca_callbacks__[1] = (e) => { \
        globalThis.log.push(`${e.type}:${e.key}:${+e.shiftKey}${+e.ctrlKey}${+e.altKey}${+e.metaKey}`); };";

    #[gpui::test]
    fn a_modifier_alone_fires_keydown_and_keyup_in_flag_order(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(inner, 'tabindex', '0'); __inca_native__.focusNode(inner);",
        );
        run_js(&mut h, MODIFIER_LOG);
        listen(&mut h, "frame", "keydown keyup", 1);
        let shift_control = Modifiers {
            shift: true,
            control: true,
            ..Modifiers::none()
        };

        for modifiers in [
            Modifiers::none(),
            Modifiers::shift(),
            Modifiers::shift(),
            shift_control,
            Modifiers::alt(),
            Modifiers::command(),
            Modifiers::none(),
        ] {
            h.cx.simulate_modifiers_change(modifiers);
        }
        h.settle();

        assert_eq!(
            log(&h),
            "keydown:Shift:1000,keydown:Control:1100,keyup:Shift:0010,keyup:Control:0010,\
             keydown:Alt:0010,keyup:Alt:0001,keydown:Meta:0001,keyup:Meta:0000"
        );
    }

    #[gpui::test]
    fn a_modifier_alone_with_nothing_focused_reaches_the_root(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            r"
            const root = __inca_native__.rootNodeId();
            globalThis.__inca_callbacks__[1] = (e) => {
                globalThis.log.push(`${e.type}:${e.currentTarget === root}:${e.target === root}`);
            };
            __inca_native__.addEventListener(root, 'keydown', 1);
            ",
        );

        h.cx.simulate_modifiers_change(Modifiers::shift());
        h.settle();

        assert_eq!(log(&h), "keydown:true:true");
    }

    #[gpui::test]
    fn a_stopped_modifier_keydown_skips_the_ancestors(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(inner, 'tabindex', '0'); __inca_native__.focusNode(inner);\
             globalThis.__inca_callbacks__[2] = (e) => { e.stopPropagation(); };",
        );
        listen(&mut h, "inner", "keydown", 2);
        listen(&mut h, "frame", "keydown", 0);

        h.cx.simulate_modifiers_change(Modifiers::shift());
        h.settle();

        assert_eq!(log(&h), "");
    }

    #[gpui::test]
    fn screen_coordinates_are_the_window_origin_plus_the_client_position(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            "globalThis.__inca_callbacks__[1] = (e) => { \
             globalThis.log.push(`${e.screenX - e.clientX}:${e.screenY - e.clientY}`); };",
        );
        listen(&mut h, "frame", "click mousemove", 1);
        let origin = h.cx.update(|window, _| window.bounds().origin);
        let inner = node(&h, "inner");

        h.click(inner);

        assert!(origin.x > px(0.0) && origin.y > px(0.0), "{origin:?}");
        let expected = format!("{}:{}", f32::from(origin.x), f32::from(origin.y));
        assert!(log(&h).ends_with(&expected), "{} vs {expected}", log(&h));
    }

    #[gpui::test]
    fn the_window_losing_and_regaining_focus_blurs_and_focuses_the_focused_node(
        cx: &mut TestAppContext,
    ) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(inner, 'tabindex', '0'); __inca_native__.focusNode(inner);\
             globalThis.__inca_callbacks__[1] = (e) => { \
             globalThis.log.push(`${e.type}:${e.relatedTarget}`); };",
        );
        listen(&mut h, "inner", "focus blur focusin focusout", 1);
        h.cx.update(|window, _| window.activate_window());
        h.settle();
        run_js(&mut h, "globalThis.log.length = 0;");

        h.cx.deactivate_window();
        h.settle();
        let lost = targets_log(&mut h);
        h.cx.update(|window, _| window.activate_window());
        h.settle();

        assert_eq!(lost, "blur:null,focusout:null");
        assert_eq!(log(&h), "focus:null,focusin:null");
    }

    #[gpui::test]
    fn a_node_made_inert_while_the_window_is_inactive_fires_no_second_blur(
        cx: &mut TestAppContext,
    ) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(inner, 'tabindex', '0'); __inca_native__.focusNode(inner);",
        );
        listen(&mut h, "inner", "focus blur", 0);
        h.cx.update(|window, _| window.activate_window());
        h.settle();
        run_js(&mut h, "globalThis.log.length = 0;");

        h.cx.deactivate_window();
        h.settle();
        run_js(
            &mut h,
            "__inca_native__.setAttribute(inner, 'inert', true);",
        );
        h.cx.update(|window, _| window.activate_window());
        h.settle();

        assert_eq!(log(&h), "blur@inner/inner/2");
    }

    #[gpui::test]
    fn focusing_in_an_inactive_window_fires_focus_when_it_activates(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(inner, 'tabindex', '0');",
        );
        listen(&mut h, "inner", "focus blur", 0);
        h.cx.update(|window, _| window.activate_window());
        h.settle();
        h.cx.deactivate_window();
        h.settle();

        run_js(&mut h, "__inca_native__.focusNode(inner);");
        let held = log(&h);
        h.cx.update(|window, _| window.activate_window());
        h.settle();

        assert_eq!(
            (held.as_str(), log(&h).as_str()),
            ("", "focus@inner/inner/2")
        );
    }

    #[gpui::test]
    fn focusing_then_blurring_in_an_inactive_window_fires_nothing(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(inner, 'tabindex', '0');",
        );
        listen(&mut h, "inner", "focus blur", 0);
        h.cx.update(|window, _| window.activate_window());
        h.settle();
        h.cx.deactivate_window();
        h.settle();

        run_js(&mut h, "__inca_native__.focusNode(inner);");
        run_js(&mut h, "__inca_native__.blurNode(inner);");
        h.cx.update(|window, _| window.activate_window());
        h.settle();

        assert_eq!(log(&h), "");
    }

    #[gpui::test]
    fn deactivating_the_window_with_nothing_focused_fires_nothing(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        listen(&mut h, "frame", "blur focusout", 0);
        h.cx.update(|window, _| window.activate_window());
        h.settle();

        h.cx.deactivate_window();
        h.settle();

        assert_eq!(log(&h), "");
    }

    /// `outer` scrolls 50 of 350 and holds `held` (50 high, tall content) and a
    /// 300 high spacer.
    fn nested_scrollers(h: &mut Harness, held_inert: bool) -> (NodeId, NodeId) {
        run_js(
            h,
            &format!(
                r"
                const n = __inca_native__;
                const box = (parent, h, scroll) => {{
                    const id = n.createNode('div');
                    n.setStyle(id, 'width', 100);
                    n.setStyle(id, 'height', h);
                    if (scroll) n.setStyle(id, 'overflow_y', 'scroll');
                    n.appendChild(parent, id);
                    return id;
                }};
                globalThis.outer = box(n.rootNodeId(), 50, true);
                globalThis.held = box(outer, 50, true);
                box(held, 300, false);
                box(outer, 300, false);
                if ({held_inert}) n.setAttribute(held, 'inert', true);
                "
            ),
        );
        (node(h, "outer"), node(h, "held"))
    }

    fn scroll_offset(h: &Harness, id: NodeId) -> Pixels {
        h.window
            .read_with(&h.cx, |app, _| {
                app.session.dispatcher.scroll_handle(id).unwrap().offset().y
            })
            .unwrap()
    }

    #[gpui::test]
    fn an_inert_scroll_container_ignores_the_wheel_and_leaves_it_to_its_ancestor(
        cx: &mut TestAppContext,
    ) {
        let mut h = Harness::load(cx, ENTRY, "");
        let (outer, held) = nested_scrollers(&mut h, true);

        h.scroll(held, 0.0, -30.0);

        assert_eq!(
            (scroll_offset(&h, held), scroll_offset(&h, outer)),
            (px(0.0), px(-30.0))
        );
    }

    #[gpui::test]
    fn a_scroll_container_that_is_not_inert_takes_the_wheel_before_its_ancestor(
        cx: &mut TestAppContext,
    ) {
        let mut h = Harness::load(cx, ENTRY, "");
        let (outer, held) = nested_scrollers(&mut h, false);

        h.scroll(held, 0.0, -30.0);

        assert_eq!(
            (scroll_offset(&h, held), scroll_offset(&h, outer)),
            (px(-30.0), px(0.0))
        );
    }

    #[gpui::test]
    fn wheel_scrolls_the_container(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        listen(&mut h, "frame", "wheel", 0);
        let (inner, mid) = (node(&h, "inner"), node(&h, "mid"));

        h.scroll(inner, 0.0, -30.0);
        let offset = h
            .window
            .read_with(&h.cx, |app, _| {
                app.session.dispatcher.scroll_handle(mid).unwrap().offset()
            })
            .unwrap();
        assert_eq!(offset.y, px(-30.0));
        assert!(targets_log(&mut h).starts_with("wheel@frame/inner/3"));
    }

    /// A > B > (C, D), each listening for the four hover events. The pointer
    /// starts outside A.
    const HOVER_ORDER: &str = r"
        const n = __inca_native__;
        const make = (parent, w, h) => {
            const id = n.createNode('div');
            n.setStyle(id, 'width', w);
            n.setStyle(id, 'height', h);
            n.appendChild(parent, id);
            return id;
        };
        const A = make(n.rootNodeId(), 200, 200);
        n.setStyle(A, 'margin_left', 50);
        const B = make(A, 100, 100);
        const C = make(B, 30, 30);
        const D = make(B, 30, 30);
        Object.assign(globalThis, { A, B, C, D });
        globalThis.log = [];
        const names = { [n.rootNodeId()]: 'root', [A]: 'A', [B]: 'B', [C]: 'C', [D]: 'D' };
        globalThis.__inca_callbacks__ = {
            0: (e) => { globalThis.log.push(`${e.type}@${names[e.currentTarget]}/${names[e.target]}/${
                e.relatedTarget === null ? 'none' : names[e.relatedTarget]}`); },
        };
        for (const id of [A, B, C, D]) {
            for (const t of ['mouseover', 'mouseout', 'mouseenter', 'mouseleave']) {
                n.addEventListener(id, t, 0);
            }
        }
    ";

    #[gpui::test]
    fn entering_a_nested_node_fires_over_then_enter_on_each_ancestor(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, HOVER_ORDER);
        let c = node(&h, "C");
        targets_log(&mut h);

        h.hover(c);

        assert_eq!(
            targets_log(&mut h),
            "mouseover@C/C/root,mouseover@B/C/root,mouseover@A/C/root,\
             mouseenter@A/A/root,mouseenter@B/B/root,mouseenter@C/C/root"
        );
    }

    #[gpui::test]
    fn the_hover_events_of_a_move_precede_its_mousemove(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, HOVER_ORDER);
        let c = node(&h, "C");
        listen(&mut h, "C", "mousemove", 0);
        targets_log(&mut h);

        h.hover(c);

        assert!(targets_log(&mut h).ends_with("mouseenter@C/C/root,mousemove@C/C/none"));
    }

    #[gpui::test]
    fn moving_to_a_sibling_fires_out_leave_over_enter_with_related_targets(
        cx: &mut TestAppContext,
    ) {
        let mut h = Harness::load(cx, ENTRY, HOVER_ORDER);
        let (c, d) = (node(&h, "C"), node(&h, "D"));
        h.hover(c);
        targets_log(&mut h);

        h.hover(d);

        assert_eq!(
            targets_log(&mut h),
            "mouseout@C/C/D,mouseout@B/C/D,mouseout@A/C/D,mouseleave@C/C/D,\
             mouseover@D/D/C,mouseover@B/D/C,mouseover@A/D/C,mouseenter@D/D/C"
        );
    }

    #[gpui::test]
    fn leaving_everything_fires_out_then_leave_from_the_inside_out(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, HOVER_ORDER);
        let d = node(&h, "D");
        h.hover(d);
        targets_log(&mut h);

        h.unhover();

        assert_eq!(
            targets_log(&mut h),
            "mouseout@D/D/none,mouseout@B/D/none,mouseout@A/D/none,\
             mouseleave@D/D/none,mouseleave@B/B/none,mouseleave@A/A/none"
        );
    }

    #[gpui::test]
    fn a_click_between_two_nodes_targets_their_common_ancestor(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        listen(&mut h, "frame", "click", 0);
        let inner = node(&h, "inner");
        h.hover(inner);
        let (down, up) = (point(px(15.0), px(15.0)), point(px(60.0), px(40.0)));

        h.cx.simulate_mouse_down(down, gpui::MouseButton::Left, gpui::Modifiers::none());
        h.cx.simulate_mouse_up(up, gpui::MouseButton::Left, gpui::Modifiers::none());
        h.settle();

        assert_eq!(targets_log(&mut h), "click@frame/mid/3");
    }

    /// Callback 1 logs `type:detail:button:buttons:clientX:clientY`.
    const POINTER_LOG: &str = "globalThis.__inca_callbacks__[1] = (e) => { \
        globalThis.log.push([e.type, e.detail, e.button, e.buttons, e.clientX, e.clientY] \
        .join(':')); };";

    fn press(h: &mut Harness, at: Point<Pixels>, button: gpui::MouseButton, count: usize) {
        h.cx.simulate_event(gpui::MouseDownEvent {
            position: at,
            button,
            click_count: count,
            ..Default::default()
        });
        h.cx.simulate_event(gpui::MouseUpEvent {
            position: at,
            button,
            click_count: count,
            ..Default::default()
        });
        h.settle();
    }

    #[gpui::test]
    fn a_mouse_click_carries_detail_button_buttons_and_coordinates(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(&mut h, POINTER_LOG);
        listen(&mut h, "frame", "click", 1);
        let inner = node(&h, "inner");
        h.hover(inner);

        press(
            &mut h,
            point(px(15.0), px(15.0)),
            gpui::MouseButton::Left,
            1,
        );

        assert_eq!(targets_log(&mut h), "click:1:0:0:15:15");
    }

    #[gpui::test]
    fn a_right_press_fires_contextmenu_after_mousedown_and_no_click(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(&mut h, POINTER_LOG);
        listen(&mut h, "inner", "mousedown", 0);
        listen(&mut h, "frame", "mousedown", 0);
        listen(&mut h, "frame", "contextmenu", 1);
        listen(&mut h, "inner", "contextmenu mouseup click", 0);
        listen(&mut h, "frame", "auxclick", 0);
        let inner = node(&h, "inner");
        h.hover(inner);

        press(
            &mut h,
            point(px(15.0), px(15.0)),
            gpui::MouseButton::Right,
            1,
        );

        assert_eq!(
            targets_log(&mut h),
            "mousedown@inner/inner/2,mousedown@frame/inner/3,contextmenu@inner/inner/2,\
             contextmenu:0:2:2:15:15,mouseup@inner/inner/2,\
             auxclick@frame/inner/3"
        );
    }

    #[gpui::test]
    fn a_stopped_mousedown_still_fires_contextmenu_and_a_middle_press_does_not(
        cx: &mut TestAppContext,
    ) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(
            &mut h,
            "globalThis.__inca_callbacks__[2] = (e) => { e.stopPropagation(); };",
        );
        listen(&mut h, "inner", "mousedown", 0);
        listen(&mut h, "inner", "mousedown", 2);
        listen(&mut h, "frame", "mousedown contextmenu click", 0);
        let inner = node(&h, "inner");
        h.hover(inner);

        press(
            &mut h,
            point(px(15.0), px(15.0)),
            gpui::MouseButton::Right,
            1,
        );
        let right = targets_log(&mut h);
        press(
            &mut h,
            point(px(15.0), px(15.0)),
            gpui::MouseButton::Middle,
            1,
        );

        assert_eq!(right, "mousedown@inner/inner/2,contextmenu@frame/inner/3");
        assert_eq!(targets_log(&mut h), "mousedown@inner/inner/2");
    }

    #[gpui::test]
    fn contextmenu_and_auxclick_reach_a_disabled_buttons_ancestors(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, BUTTON);
        run_js(
            &mut h,
            "__inca_native__.setAttribute(box, 'disabled', true);",
        );
        listen(&mut h, "frame", "mousedown contextmenu auxclick", 0);
        let button = node(&h, "box");
        h.hover(button);

        press(
            &mut h,
            point(px(10.0), px(10.0)),
            gpui::MouseButton::Right,
            1,
        );

        assert_eq!(log(&h), "contextmenu@frame/box,auxclick@frame/box");
    }

    #[gpui::test]
    fn auxclick_carries_the_released_button_and_coordinates(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(&mut h, POINTER_LOG);
        listen(&mut h, "frame", "auxclick", 1);
        let inner = node(&h, "inner");
        h.hover(inner);
        let mut got = Vec::new();

        for button in [
            gpui::MouseButton::Left,
            gpui::MouseButton::Right,
            gpui::MouseButton::Middle,
        ] {
            press(&mut h, point(px(15.0), px(15.0)), button, 1);
            got.push(targets_log(&mut h));
        }

        assert_eq!(got, ["", "auxclick:1:2:0:15:15", "auxclick:1:1:0:15:15"]);
    }

    #[gpui::test]
    fn a_double_click_fires_dblclick_after_the_second_click(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TARGETS);
        run_js(&mut h, POINTER_LOG);
        listen(&mut h, "frame", "click dblclick", 1);
        listen(&mut h, "mid", "dblclick", 0);
        let inner = node(&h, "inner");
        h.hover(inner);

        for count in 1..=3 {
            press(
                &mut h,
                point(px(15.0), px(15.0)),
                gpui::MouseButton::Left,
                count,
            );
        }

        let plain = "1:0:0:15:15";
        assert_eq!(
            targets_log(&mut h),
            format!(
                "click:{plain},dblclick@mid/inner/3,click:2:0:0:15:15,dblclick:2:0:0:15:15,click:3:0:0:15:15"
            )
        );
    }

    /// A `frame` of 10x10 nodes `n3`, `n1`, `z1`, `z2`, `m1` and `n2` with
    /// `tabindex` 3, 1, 0, 0, -1 and 2, and `mk(name, tabindex, tag)` for more.
    /// Callback 0 logs `type@target/relatedTarget`, 1 calls `preventDefault`,
    /// 2 calls `stopPropagation`, 4 logs `type@target`. `watch(4)` listens to
    /// `focus` and `blur` on every node made so far.
    const TABS: &str = r"
        const n = __inca_native__;
        const names = {};
        const nm = (id) => names[id] ?? 'null';
        const frame = n.createNode('div');
        n.setStyle(frame, 'width', 200);
        n.setStyle(frame, 'height', 200);
        n.appendChild(n.rootNodeId(), frame);
        globalThis.frame = frame;
        globalThis.mk = (name, tabindex, tag = 'div') => {
            const id = n.createNode(tag);
            n.setStyle(id, 'width', 10);
            n.setStyle(id, 'height', 10);
            if (tabindex !== null) n.setAttribute(id, 'tabindex', String(tabindex));
            n.appendChild(frame, id);
            names[id] = name;
            globalThis[name] = id;
            return id;
        };
        for (const [name, index] of [['n3', 3], ['n1', 1], ['z1', 0], ['z2', 0], ['m1', -1], ['n2', 2]]) {
            mk(name, index);
        }
        globalThis.log = [];
        globalThis.__inca_callbacks__ = {
            0: (e) => { globalThis.log.push(`${e.type}@${nm(e.target)}/${nm(e.relatedTarget)}`); },
            1: (e) => { e.preventDefault(); },
            2: (e) => { e.stopPropagation(); },
            4: (e) => { globalThis.log.push(`${e.type}@${nm(e.target)}`); },
        };
        globalThis.watch = (callback, types = ['focus', 'blur']) => {
            for (const id of Object.keys(names)) {
                for (const type of types) n.addEventListener(Number(id), type, callback);
            }
        };
    ";

    fn tabs(cx: &mut TestAppContext) -> Harness {
        let mut h = Harness::load(cx, ENTRY, TABS);
        run_js(&mut h, "watch(4);");
        h
    }

    #[gpui::test]
    fn tab_follows_the_order_and_wraps_through_nothing(cx: &mut TestAppContext) {
        let mut h = tabs(cx);
        for _ in 0..7 {
            h.keystrokes("tab");
        }
        assert_eq!(
            log(&h),
            "focus@n1,blur@n1,focus@n2,blur@n2,focus@n3,blur@n3,focus@z1,blur@z1,focus@z2,\
             blur@z2,focus@n1"
        );
    }

    #[gpui::test]
    fn shift_tab_walks_backward_and_wraps_through_nothing(cx: &mut TestAppContext) {
        let mut h = tabs(cx);
        for _ in 0..7 {
            h.keystrokes("shift-tab");
        }
        assert_eq!(
            log(&h),
            "focus@z2,blur@z2,focus@z1,blur@z1,focus@n3,blur@n3,focus@n2,blur@n2,focus@n1,\
             blur@n1,focus@z2"
        );
    }

    #[gpui::test]
    fn tab_from_a_negative_tabindex_continues_in_tree_order(cx: &mut TestAppContext) {
        let mut h = tabs(cx);
        run_js(&mut h, "__inca_native__.focusNode(m1);");
        h.keystrokes("tab");
        run_js(&mut h, "__inca_native__.focusNode(m1);");
        h.keystrokes("shift-tab");
        assert_eq!(
            log(&h),
            "focus@m1,blur@m1,focus@n2,blur@n2,focus@m1,blur@m1,focus@z2"
        );
    }

    #[gpui::test]
    fn focus_events_of_a_tab_move_carry_related_targets(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TABS);
        run_js(
            &mut h,
            "watch(0, ['focus', 'blur', 'focusin', 'focusout']);",
        );
        h.keystrokes("tab tab");
        assert_eq!(
            log(&h),
            "focus@n1/null,focusin@n1/null,blur@n1/n2,focusout@n1/n2,focus@n2/n1,focusin@n2/n1"
        );
    }

    #[gpui::test]
    fn a_prevented_key_down_cancels_the_tab_move(cx: &mut TestAppContext) {
        let mut h = tabs(cx);
        run_js(&mut h, "__inca_native__.focusNode(n1);");
        run_js(
            &mut h,
            "__inca_native__.addEventListener(__inca_native__.rootNodeId(), 'keydown', 1);",
        );
        h.keystrokes("tab shift-tab");
        assert_eq!(log(&h), "focus@n1");
    }

    #[gpui::test]
    fn stopping_a_key_down_keeps_the_tab_move(cx: &mut TestAppContext) {
        let mut h = tabs(cx);
        run_js(&mut h, "__inca_native__.focusNode(n1);");
        run_js(
            &mut h,
            "__inca_native__.addEventListener(n1, 'keydown', 4); \
             __inca_native__.addEventListener(n1, 'keydown', 2);",
        );
        h.keystrokes("tab");
        assert_eq!(log(&h), "focus@n1,keydown@n1,blur@n1,focus@n2");
    }

    #[gpui::test]
    fn tab_moves_after_a_prevented_mouse_down(cx: &mut TestAppContext) {
        let mut h = tabs(cx);
        run_js(
            &mut h,
            "__inca_native__.addEventListener(frame, 'mousedown', 1);",
        );
        let frame = node(&h, "frame");
        h.click(frame);
        h.keystrokes("tab");
        assert_eq!(log(&h), "focus@n1");
    }

    #[gpui::test]
    fn tab_after_a_click_on_an_empty_area_focuses_the_first_node(cx: &mut TestAppContext) {
        let mut h = tabs(cx);
        run_js(&mut h, "__inca_native__.focusNode(n2);");
        let frame = node(&h, "frame");
        h.click(frame);
        h.keystrokes("tab");
        assert_eq!(log(&h), "focus@n2,blur@n2,focus@n1");
    }

    #[gpui::test]
    fn control_alt_and_cmd_tab_leave_focus_alone(cx: &mut TestAppContext) {
        let mut h = tabs(cx);
        run_js(&mut h, "__inca_native__.focusNode(n1);");
        h.keystrokes("ctrl-tab alt-tab cmd-tab ctrl-shift-tab");
        assert_eq!(log(&h), "focus@n1");
    }

    #[gpui::test]
    fn tab_skips_a_disabled_button_and_a_hidden_node_and_leaves_a_button(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, TABS);
        run_js(
            &mut h,
            "const n = __inca_native__; \
             n.setAttribute(mk('bd', 1, 'button'), 'disabled', true); \
             n.setAttribute(mk('hid', 1), 'hidden', true); \
             mk('bt', null, 'button'); watch(4);",
        );
        for _ in 0..7 {
            h.keystrokes("tab");
        }
        assert_eq!(
            log(&h),
            "focus@n1,blur@n1,focus@n2,blur@n2,focus@n3,blur@n3,focus@z1,blur@z1,focus@z2,\
             blur@z2,focus@bt,blur@bt"
        );
    }
}
