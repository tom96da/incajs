// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pointer and wheel input of the [`super::Harness`].

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, ScrollDelta,
    ScrollWheelEvent, point, px,
};
use inca_gpui::{NodeId, debug_selector};

use super::Harness;
use crate::snapshot::{Bounds, Node};

impl Harness {
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

    pub(super) fn move_pointer(&mut self, at: Point<Pixels>) {
        self.cx.simulate_mouse_move(at, None, Modifiers::none());
        self.settle();
    }

    pub(super) fn bounds_of(&mut self, id: NodeId) -> Option<Bounds> {
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

    pub(super) fn center_of(&mut self, id: NodeId) -> Point<Pixels> {
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
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::tests::{TEST_ENTRY_PATH, eval_f64, id_where, log, node};
    use gpui::TestAppContext;
    use std::collections::BTreeMap;

    /// A 100x50 box holding a text leaf, with a click listener that counts.
    const CLICKABLE: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.appendChild(n.rootNodeId(), box);
            const label = n.createNode('#text');
            n.setAttribute(label, 'value', 'hi');
            n.appendChild(box, label);
            globalThis.clicks = 0;
            globalThis.__inca_callbacks__[0] = () => {
                globalThis.clicks++;
            };
            n.addEventListener(box, 'click');
        ";

    #[gpui::test]
    fn load_settles_the_layout_to_the_viewport(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, CLICKABLE);

        let width = eval_f64(
            &h,
            "__inca_native__.layoutOf(__inca_native__.rootNodeId())[2]",
        );
        let viewport =
            h.cx.update(|window, _| f64::from(f32::from(window.viewport_size().width)));

        assert_eq!(width, viewport);
    }

    #[gpui::test]
    fn load_mounts_the_bundle_and_reports_bounds(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, CLICKABLE);

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
        Harness::load(cx, TEST_ENTRY_PATH, "throw new Error('boom');");
    }

    #[gpui::test]
    fn click_reaches_a_listener_on_the_node(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, CLICKABLE);
        let boxed = id_where(&mut h, |n| {
            n.tag == "div" && n.bounds.is_some_and(|b| b.width == 100.0)
        });

        h.click(boxed);

        assert_eq!(eval_f64(&h, "globalThis.clicks"), 1.0);
    }

    #[gpui::test]
    fn click_on_a_text_leaf_lands_on_its_container(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, CLICKABLE);
        let leaf = id_where(&mut h, |n| n.text.is_some());

        h.click(leaf);

        assert_eq!(eval_f64(&h, "globalThis.clicks"), 1.0);
    }

    #[gpui::test]
    #[should_panic(expected = "no node 9999 in the tree")]
    fn click_panics_on_an_unknown_node(cx: &mut TestAppContext) {
        Harness::load(cx, TEST_ENTRY_PATH, CLICKABLE).click(9999);
    }

    #[gpui::test]
    #[should_panic(expected = "the root node exists")]
    fn snapshot_panics_without_a_root_node(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, "");
        let root = h.host.borrow().root();
        h.host.borrow_mut().tree.destroy_node(root);
        h.snapshot();
    }

    /// A 100x50 box whose auxclick listener logs `detail:shiftKey:button`.
    const CLICK_FIELDS: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.appendChild(n.rootNodeId(), box);
            globalThis.box = box;
            globalThis.log = [];
            globalThis.__inca_callbacks__[0] = (e) => {
                globalThis.log.push(`${e.detail}:${e.shiftKey}:${e.button}`);
            };
            n.addEventListener(box, 'auxclick');
        ";

    #[gpui::test]
    fn click_with_passes_its_arguments_to_the_click(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, CLICK_FIELDS);
        let boxed = node(&h, "box");

        h.click_with(boxed, gpui::MouseButton::Right, Modifiers::shift(), 2);

        assert_eq!(log(&h), "2:true:2");
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
            globalThis.__inca_callbacks__[0] = () => {
                globalThis.enters++;
            };
            globalThis.__inca_callbacks__[1] = () => {
                globalThis.leaves++;
            };
            n.addEventListener(box, 'mouseenter');
            n.addEventListener(box, 'mouseleave');
        ";

    fn hover_box(h: &mut Harness) -> NodeId {
        id_where(h, |n| n.bounds.is_some_and(|b| b.width == 100.0))
    }

    #[gpui::test]
    fn hover_fires_mouseenter_once(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, HOVERABLE);
        let boxed = hover_box(&mut h);
        assert_eq!(eval_f64(&h, "globalThis.enters"), 0.0);

        h.hover(boxed);

        assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
        assert_eq!(eval_f64(&h, "globalThis.leaves"), 0.0);
    }

    #[gpui::test]
    fn hover_again_after_leaving_fires_mouseenter_again(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, HOVERABLE);
        let boxed = hover_box(&mut h);

        h.hover(boxed);
        h.unhover();
        h.hover(boxed);

        assert_eq!(eval_f64(&h, "globalThis.enters"), 2.0);
        assert_eq!(eval_f64(&h, "globalThis.leaves"), 1.0);
    }

    #[gpui::test]
    fn unhover_before_any_hover_fires_nothing(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            TEST_ENTRY_PATH,
            r"
                const n = __inca_native__;
                const box = n.createNode('div');
                n.setStyle(box, 'width', 100);
                n.setStyle(box, 'height', 50);
                n.setStyle(box, 'margin_left', 200);
                n.appendChild(n.rootNodeId(), box);
                globalThis.leaves = 0;
                globalThis.__inca_callbacks__[0] = () => {
                    globalThis.leaves++;
                };
                n.addEventListener(box, 'mouseleave');
                ",
        );

        h.unhover();

        assert_eq!(eval_f64(&h, "globalThis.leaves"), 0.0);
    }

    #[gpui::test]
    fn hover_on_a_text_leaf_lands_on_its_container(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            TEST_ENTRY_PATH,
            &format!(
                r"
                {HOVERABLE}
                const label = n.createNode('#text');
                n.setAttribute(label, 'value', 'hi');
                n.appendChild(box, label);
                "
            ),
        );
        let leaf = id_where(&mut h, |n| n.text.is_some());

        h.hover(leaf);

        assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
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
            TEST_ENTRY_PATH,
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
}
