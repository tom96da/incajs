// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Drives a mounted app headlessly: reads its tree, sends input and waits
//! for it to go idle.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    Modifiers, Pixels, Point, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext,
    WindowHandle, point, px,
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
    /// focused node.
    ///
    /// # Panics
    ///
    /// Panics when a keystroke does not parse.
    pub fn keystrokes(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
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
            n.focusNode(box);
            ",
        );

        h.keystrokes("a b");

        assert_eq!(eval_f64(&h, "globalThis.keys"), 2.0);
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

    #[gpui::test]
    #[should_panic(expected = "the window is open")]
    fn settle_panics_once_the_window_has_closed(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, ENTRY, "");
        h.cx.update(|window, _| window.remove_window());
        h.settle();
    }
}
