// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pointer and wheel input of the [`super::Harness`].

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, ScrollDelta, ScrollEvent,
    TouchPhase, point, px,
};
use inca_gpui::debug_selector;
use inca_node::NodeId;

use super::Harness;
use crate::snapshot::{Bounds, Lookup};

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
        self.move_to(position, None, Modifiers::none());
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
        self.move_to(at, None, Modifiers::none());
    }

    /// Moves the pointer to a point outside every node, so each node it was
    /// over receives `mouseleave`.
    pub fn unhover(&mut self) {
        self.move_to(point(px(-1.0), px(-1.0)), None, Modifiers::none());
    }

    /// Scrolls with a pixel wheel event of (`dx`, `dy`) over node `id`,
    /// located as in [`Harness::click`]. A negative `dy` scrolls down.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Harness::click`].
    pub fn scroll(&mut self, id: NodeId, dx: f32, dy: f32) {
        let position = self.center_of(id);
        self.cx.simulate_event(ScrollEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
            ..Default::default()
        });
        self.settle();
    }

    /// Scrolls with a wheel event of `delta`, in pixels or in lines, and
    /// `modifiers` held, over node `id`, located as in [`Harness::click`]. A
    /// negative `y` scrolls down.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as [`Harness::click`].
    pub fn scroll_with(&mut self, id: NodeId, delta: ScrollDelta, modifiers: Modifiers) {
        let position = self.center_of(id);
        self.cx.simulate_event(ScrollEvent {
            position,
            delta,
            modifiers,
            touch_phase: TouchPhase::Moved,
            ..Default::default()
        });
        self.settle();
    }

    /// Moves the pointer to `at` with `modifiers` held, as one platform
    /// mouse-move event on every call, also when the pointer is already at
    /// `at`. `pressed` is the button the platform reports held during the
    /// move. With `None` the move ends the `:active` state of a held press and
    /// a scrollbar thumb drag. The `buttons` an app reads is the host's own
    /// record of the presses, so `pressed` does not change it.
    pub fn move_to(
        &mut self,
        at: Point<Pixels>,
        pressed: Option<MouseButton>,
        modifiers: Modifiers,
    ) {
        self.cx.simulate_mouse_move(at, pressed, modifiers);
        self.settle();
    }

    /// Presses `button` at `at` with `modifiers` held, as a single click.
    /// When the pointer is not at `at`, a move to `at` with no pressed button
    /// comes first, also while a button is held from an earlier press. The
    /// pointer starts at `(0, 0)` after [`Harness::load`], so a press there
    /// sends no move.
    pub fn mouse_down(&mut self, at: Point<Pixels>, button: MouseButton, modifiers: Modifiers) {
        if self.pointer() != at {
            self.move_to(at, None, modifiers);
        }
        self.cx.simulate_event(MouseDownEvent {
            position: at,
            button,
            modifiers,
            click_count: 1,
            first_mouse: false,
        });
        self.settle();
    }

    /// Releases `button` at `at` with `modifiers` held. When the pointer is
    /// not at `at`, a move to `at` with `button` pressed comes first. It
    /// expects an earlier [`Harness::mouse_down`] of `button` and does not
    /// check.
    pub fn mouse_up(&mut self, at: Point<Pixels>, button: MouseButton, modifiers: Modifiers) {
        if self.pointer() != at {
            self.move_to(at, Some(button), modifiers);
        }
        self.cx.simulate_event(MouseUpEvent {
            position: at,
            button,
            modifiers,
            click_count: 1,
        });
        self.settle();
    }

    fn pointer(&mut self) -> Point<Pixels> {
        self.cx.update(|window, _| window.mouse_position())
    }

    pub(super) fn bounds_of(&mut self, id: NodeId) -> Option<Bounds> {
        let selector = *self
            .selectors
            .entry(id)
            .or_insert_with(|| Box::leak(debug_selector(id).into_boxed_str()));
        self.cx.debug_bounds(selector).map(Bounds::from)
    }

    pub(super) fn center_of(&mut self, id: NodeId) -> Point<Pixels> {
        let root = self.snapshot();
        match root.nearest_bounds(id) {
            Lookup::At(bounds) => bounds.center(),
            Lookup::Absent => panic!("no node {id} in the tree"),
            Lookup::Unbounded => panic!("node {id} and its ancestors have no bounds"),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::tests::{TEST_ENTRY_PATH, eval_f64, id_where, load_listening, log, node};
    use gpui::TestAppContext;

    /// A 100x50 box holding a text leaf, with a click listener that counts.
    const CLICKABLE: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.appendChild(n.rootNodeId(), box);
            const label = n.createNode('#text');
            n.setCharacterData(label, 'hi');
            n.appendChild(box, label);
            globalThis.clicks = 0;
            n.addEventListener(box, 'click', () => {
                globalThis.clicks++;
            });
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
        h.host.document.borrow_mut().tree.set_root(9999);
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
            n.addEventListener(box, 'auxclick', (e) => {
                globalThis.log.push(`${e.detail}:${e.shiftKey}:${e.button}`);
            });
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
            n.addEventListener(box, 'mouseenter', () => {
                globalThis.enters++;
            });
            n.addEventListener(box, 'mouseleave', () => {
                globalThis.leaves++;
            });
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
                n.addEventListener(box, 'mouseleave', () => {
                    globalThis.leaves++;
                });
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
                n.setCharacterData(label, 'hi');
                n.appendChild(box, label);
                "
            ),
        );
        let leaf = id_where(&mut h, |n| n.text.is_some());

        h.hover(leaf);

        assert_eq!(eval_f64(&h, "globalThis.enters"), 1.0);
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

    /// A 100x50 `box` that logs `type:button:buttons:shiftKey` for `mousemove`, `mousedown` and
    /// `mouseup`.
    const POINTED: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.appendChild(n.rootNodeId(), box);
            globalThis.log = [];
            globalThis.cb = {
                0: (e) => {
                    globalThis.log.push(`${e.type}:${e.button}:${e.buttons}:${e.shiftKey}`);
                },
            };
            listen(box, 'mousemove', 0);
            listen(box, 'mousedown', 0);
            listen(box, 'mouseup', 0);
        ";

    /// `POINTED` logging `type:detail` for `mousedown` only.
    const COUNTED: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.appendChild(n.rootNodeId(), box);
            globalThis.log = [];
            globalThis.cb = {
                0: (e) => {
                    globalThis.log.push(`${e.type}:${e.detail}`);
                },
            };
            listen(box, 'mousedown', 0);
        ";

    /// A 100x100 `box` with a vertical scrollbar over a 300x300 `inner`.
    const THUMBED: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 100);
            n.setStyle(box, 'overflow_x', 'hidden');
            n.setStyle(box, 'overflow_y', 'scroll');
            n.appendChild(n.rootNodeId(), box);
            const inner = n.createNode('div');
            n.setStyle(inner, 'width', 300);
            n.setStyle(inner, 'height', 300);
            n.appendChild(box, inner);
            globalThis.inner = inner;
        ";

    fn box_bounds(h: &mut Harness) -> Bounds {
        h.snapshot().children[0].bounds.unwrap()
    }

    fn center(h: &mut Harness) -> Point<Pixels> {
        box_bounds(h).center()
    }

    /// The point on the thumb of a borderless container scrolled to 0, with a classic bar.
    fn thumb(h: &mut Harness) -> Point<Pixels> {
        let b = box_bounds(h);
        point(px(b.x + b.width - 8.0), px(b.y + 10.0))
    }

    fn below(at: Point<Pixels>, by: f32) -> Point<Pixels> {
        point(at.x, at.y + px(by))
    }

    fn y_inner(h: &mut Harness) -> f32 {
        let inner = node(h, "inner");
        h.snapshot()
            .find(&|n| n.id == inner)
            .unwrap()
            .bounds
            .unwrap()
            .y
    }

    #[gpui::test]
    fn mouse_down_from_elsewhere_moves_then_presses(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, POINTED);
        let c = center(&mut h);

        h.mouse_down(c, MouseButton::Left, Modifiers::none());

        assert_eq!(log(&h), "mousemove:0:0:false,mousedown:0:1:false");
    }

    #[gpui::test]
    fn mouse_down_at_the_pointer_sends_no_move(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, POINTED);
        let c = center(&mut h);
        h.move_to(c, None, Modifiers::none());

        h.mouse_down(c, MouseButton::Left, Modifiers::none());

        assert_eq!(log(&h), "mousemove:0:0:false,mousedown:0:1:false");
    }

    #[gpui::test]
    fn mouse_down_passes_its_button_and_modifiers(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, POINTED);
        let c = center(&mut h);

        h.mouse_down(c, MouseButton::Right, Modifiers::shift());

        assert_eq!(log(&h), "mousemove:0:0:true,mousedown:2:2:true");
    }

    #[gpui::test]
    fn mouse_up_releases_the_button(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, POINTED);
        let c = center(&mut h);
        h.mouse_down(c, MouseButton::Left, Modifiers::none());

        h.mouse_up(c, MouseButton::Left, Modifiers::none());

        assert_eq!(
            log(&h),
            "mousemove:0:0:false,mousedown:0:1:false,mouseup:0:0:false"
        );
    }

    #[gpui::test]
    fn mouse_up_elsewhere_moves_with_the_button_pressed(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, THUMBED);
        let t = thumb(&mut h);
        let before = y_inner(&mut h);
        h.mouse_down(t, MouseButton::Left, Modifiers::none());

        h.mouse_up(below(t, 20.0), MouseButton::Left, Modifiers::none());

        assert!(y_inner(&mut h) < before);
    }

    #[gpui::test]
    fn mouse_up_passes_its_button_and_modifiers(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, POINTED);
        let c = center(&mut h);
        h.mouse_down(c, MouseButton::Right, Modifiers::shift());

        h.mouse_up(c, MouseButton::Right, Modifiers::shift());

        assert_eq!(
            log(&h),
            "mousemove:0:0:true,mousedown:2:2:true,mouseup:2:0:true"
        );
    }

    #[gpui::test]
    fn move_to_with_a_pressed_button_drags_the_thumb(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, THUMBED);
        let t = thumb(&mut h);
        let before = y_inner(&mut h);
        h.mouse_down(t, MouseButton::Left, Modifiers::none());

        h.move_to(below(t, 20.0), Some(MouseButton::Left), Modifiers::none());

        assert!(y_inner(&mut h) < before);
    }

    #[gpui::test]
    fn mouse_down_elsewhere_moves_with_no_button_pressed(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, THUMBED);
        let t = thumb(&mut h);
        let before = y_inner(&mut h);
        h.mouse_down(t, MouseButton::Left, Modifiers::none());

        h.mouse_down(below(t, 20.0), MouseButton::Right, Modifiers::none());

        assert_eq!(y_inner(&mut h), before);
    }

    #[gpui::test]
    fn move_to_passes_its_modifiers(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, POINTED);
        let c = center(&mut h);

        h.move_to(c, None, Modifiers::shift());

        assert_eq!(log(&h), "mousemove:0:0:true");
    }

    #[gpui::test]
    fn move_to_the_same_point_moves_again(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, POINTED);
        let c = center(&mut h);

        h.move_to(c, None, Modifiers::none());
        h.move_to(c, None, Modifiers::none());

        assert_eq!(log(&h), "mousemove:0:0:false,mousemove:0:0:false");
    }

    #[gpui::test]
    fn mouse_down_sends_a_single_click(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, COUNTED);
        let c = center(&mut h);

        h.mouse_down(c, MouseButton::Left, Modifiers::none());

        assert_eq!(log(&h), "mousedown:1");
    }

    #[gpui::test]
    fn mouse_up_elsewhere_moves_with_its_modifiers(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, POINTED);
        let c = center(&mut h);
        h.mouse_down(c, MouseButton::Left, Modifiers::none());

        h.mouse_up(
            point(c.x + px(10.0), c.y),
            MouseButton::Left,
            Modifiers::shift(),
        );

        assert_eq!(
            log(&h),
            "mousemove:0:0:false,mousedown:0:1:false,mousemove:0:1:true,mouseup:0:0:true"
        );
    }

    #[gpui::test]
    fn mouse_down_at_the_start_point_sends_no_move(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, POINTED);
        let b = box_bounds(&mut h);
        assert_eq!((b.x, b.y), (0.0, 0.0));

        h.mouse_down(Point::default(), MouseButton::Left, Modifiers::none());

        assert_eq!(log(&h), "mousedown:0:1:false");
    }

    /// A 100x50 scrollable box holding a 100x200 child, with a wheel listener
    /// that logs `deltaY > 0`, `ctrlKey` and `deltaMode`.
    const WHEELED: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.setStyle(box, 'overflow_y', 'scroll');
            n.appendChild(n.rootNodeId(), box);
            const child = n.createNode('div');
            n.setStyle(child, 'width', 100);
            n.setStyle(child, 'height', 200);
            n.appendChild(box, child);
            globalThis.box = box;
            globalThis.log = [];
            globalThis.cb = {
                0: (e) => {
                    globalThis.log.push(`${e.deltaY > 0}:${e.ctrlKey}:${e.deltaMode}`);
                },
            };
            listen(box, 'wheel', 0);
        ";

    #[gpui::test]
    fn scroll_with_passes_its_delta_and_modifiers(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, WHEELED);
        let boxed = node(&h, "box");

        h.scroll_with(
            boxed,
            ScrollDelta::Lines(point(0.0, -2.0)),
            Modifiers::control(),
        );

        assert_eq!(log(&h), "true:true:1");
    }

    #[gpui::test]
    fn mouse_down_after_an_activation_moves_again(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, POINTED);
        let c = center(&mut h);

        h.move_to(c, None, Modifiers::none());
        h.activate_window();
        h.mouse_down(c, MouseButton::Left, Modifiers::none());

        assert_eq!(
            log(&h),
            "mousemove:0:0:false,mousemove:0:0:false,mousedown:0:1:false"
        );
    }
}
