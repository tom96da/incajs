// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Drives a mounted app headlessly: reads its tree, sends input and waits
//! for it to go idle.

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, PlatformInput, TestAppContext,
    VisualTestContext, WindowHandle,
};
use inca_bridge::Host;
pub use inca_gpui::Scrollbars;
use inca_jsenv::stderr_reporter;
use inca_node::NodeId;

use crate::app::{HostedApp, start};
use crate::snapshot::{Node, snapshot};

/// An app mounted in a test window.
///
/// The window, config, menu and sizing are the ones `run_bundle` sets up.
/// Every input method settles the app before it returns, so a following
/// [`Harness::snapshot`] sees the result.
pub struct Harness {
    pub(crate) cx: VisualTestContext,
    pub(crate) window: WindowHandle<HostedApp>,
    host: Rc<Host>,
    // Each selector string is leaked once per node id, because
    // `debug_bounds` takes `&'static str`.
    selectors: HashMap<NodeId, &'static str>,
}

mod pointer;
mod text;

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
        let host = host.document.borrow();
        snapshot(&host.tree, &host.style, host.root(), &mut |id| {
            self.bounds_of(id)
        })
        .expect("the root node exists")
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
        let os = self.cx.update(|_, cx| cx.platform_info().os);
        for keystroke in keys.split(' ').map(|key| Keystroke::parse_with_os(key, os)) {
            self.cx.simulate_event(KeyUpEvent {
                keystroke: keystroke.expect("a parseable keystroke"),
            });
        }
        self.settle();
    }

    /// Sends space-separated keystrokes, such as `"a b"`, to the focused node
    /// as keys the platform repeats. Each key-down has `repeat` true. When no
    /// listener cancels it and the keystroke carries text, that text goes to the
    /// focused `input`. No key-up is sent.
    ///
    /// # Panics
    ///
    /// Panics when a keystroke does not parse.
    pub fn key_repeat(&mut self, keys: &str) {
        let os = self.cx.update(|_, cx| cx.platform_info().os);
        for keystroke in keys.split(' ').map(|key| Keystroke::parse_with_os(key, os)) {
            let keystroke = keystroke
                .expect("a parseable keystroke")
                .with_simulated_ime();
            let text = keystroke.key_char.clone();
            let result = self.cx.update(|window, cx| {
                window.dispatch_event(
                    PlatformInput::KeyDown(KeyDownEvent {
                        keystroke,
                        is_held: true,
                        prefer_character_input: false,
                    }),
                    cx,
                )
            });
            if let (true, Some(text)) = (result.propagate, text) {
                self.type_text(&text);
            }
        }
        self.settle();
    }

    /// Reports `modifiers` as the modifier keys held now, with caps lock off,
    /// until the next activation or deactivation. [`Modifiers::none`] reports
    /// every key released. No key event is sent apart from the ones the app
    /// derives from the change.
    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.cx.simulate_modifiers_change(modifiers);
        self.settle();
    }

    /// Reports the window active. The app sees the activation: `window`
    /// receives `focus`, and the focused node receives `focus` and `focusin`
    /// again. The held modifier keys are reported released, with no key
    /// event.
    ///
    /// [`Harness::load`] leaves the window inactive.
    ///
    /// # Panics
    ///
    /// Panics when the window is already active.
    pub fn activate_window(&mut self) {
        assert!(!self.is_window_active(), "the window is already active");
        self.cx.update(|window, _| window.activate_window());
        self.settle();
    }

    /// Reports the window inactive. The app sees the deactivation: `window`
    /// receives `blur`. The held modifier keys are reported released, with
    /// no key event.
    ///
    /// # Panics
    ///
    /// Panics when the window is not active.
    pub fn deactivate_window(&mut self) {
        assert!(self.is_window_active(), "the window is not active");
        self.cx.deactivate_window();
        self.settle();
    }

    fn is_window_active(&mut self) -> bool {
        self.cx.update(|window, _| window.is_window_active())
    }

    /// Runs one pass: the app's pending JS jobs, then queued work until none
    /// is left, then the frame callbacks the app requested, then a window
    /// draw, so [`Harness::snapshot`] reads current bounds. Jobs scheduled by
    /// that draw and frame callbacks requested by a frame callback run on the
    /// next call.
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
        self.window
            .update(&mut self.cx, |app, window, cx| {
                if app.session.host.js.borrow().frames.is_pending() {
                    app.session.dispatcher.run_frame_callbacks(window, cx);
                }
            })
            .expect("the window is open");
        self.cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// Makes every render after this call take `mode` as the platform's scrollbar setting, until
    /// the next call, then settles. The test platform shows classic bars until this is called.
    pub fn scrollbars(&mut self, mode: Scrollbars) {
        self.cx.update(|_, cx| Scrollbars::set_for_tests(cx, mode));
        self.settle();
    }

    /// Moves the fake clock forward by `by`, runs the timers that fall due, then
    /// settles. A repeating timer runs once per call, whatever `by` is.
    pub fn advance_clock(&mut self, by: Duration) {
        self.cx.executor().advance_clock(by);
        self.settle();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::tests::{TEST_ENTRY_PATH, eval_f64, id_where, load_listening, log, node};
    use gpui::{point, px};

    #[gpui::test]
    fn settle_flushes_a_microtask_mount(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, "");
        let before = h.snapshot().children.len();
        h.window
            .read_with(&h.cx, |app, _| {
                app.session
                    .engine
                    .eval::<()>(
                        r"
                        Promise.resolve().then(() => {
                            const n = __inca_native__;
                            const d = n.createNode('div');
                            n.setStyle(d, 'width', 40);
                            n.setStyle(d, 'height', 20);
                            n.appendChild(n.rootNodeId(), d);
                        });
                        ",
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
            const label = n.createNode('#text');
            n.setCharacterData(label, 'hi');
            n.appendChild(c, label);
            const inner = n.createNode('div');
            n.setStyle(inner, 'width', 300);
            n.setStyle(inner, 'height', 300);
            n.appendChild(c, inner);
            globalThis.inner = inner;
            globalThis.log = [];
            globalThis.cb = {
                0: (e) => {
                    globalThis.log.push(`${e.deltaX}:${e.deltaY}`);
                },
            };
            listen(c, 'wheel', 0);
        ";

    fn id_y(h: &mut Harness, id: NodeId) -> f32 {
        h.snapshot()
            .find(&|n| n.id == id)
            .unwrap()
            .bounds
            .unwrap()
            .y
    }

    #[gpui::test]
    fn scroll_on_a_text_leaf_scrolls_its_container(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, SCROLLER);
        let leaf = id_where(&mut h, |n| n.text.is_some());
        let inner = node(&h, "inner");
        let before = id_y(&mut h, inner);

        h.scroll(leaf, 0.0, -20.0);

        assert_eq!(id_y(&mut h, inner), before - 20.0);
    }

    #[gpui::test]
    fn scroll_passes_both_deltas_to_the_wheel_event(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, SCROLLER);
        let container = id_where(&mut h, |n| n.children.iter().any(|c| c.text.is_some()));

        h.scroll(container, -25.0, -20.0);

        assert_eq!(log(&h), "25:20");
    }

    #[gpui::test]
    fn click_aims_at_content_scrolled_out_of_its_viewport(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            TEST_ENTRY_PATH,
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
            TEST_ENTRY_PATH,
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
        Harness::load(cx, TEST_ENTRY_PATH, "").keystrokes("a-b");
    }

    #[gpui::test]
    #[should_panic(expected = "InvalidKeystrokeError")]
    fn key_up_panics_on_an_unparseable_key(cx: &mut TestAppContext) {
        Harness::load(cx, TEST_ENTRY_PATH, "").key_up("a-b");
    }

    /// A focused 100x50 `box` whose `keydown` and `keyup` listeners log `type:key:metaKey`.
    pub(super) const KEYED: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.setAttribute(box, 'tabindex', '0');
            n.appendChild(n.rootNodeId(), box);
            globalThis.log = [];
            globalThis.cb = {
                0: (e) => {
                    globalThis.log.push(`${e.type}:${e.key}:${e.metaKey}`);
                },
            };
            listen(box, 'keydown', 0);
            listen(box, 'keyup', 0);
            n.focusNode(box);
        ";

    /// `KEYED` with the listeners logging `type:key:repeat`.
    const REPEATED: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.setAttribute(box, 'tabindex', '0');
            n.appendChild(n.rootNodeId(), box);
            globalThis.log = [];
            globalThis.cb = {
                0: (e) => {
                    globalThis.log.push(`${e.type}:${e.key}:${e.repeat}`);
                },
            };
            listen(box, 'keydown', 0);
            listen(box, 'keyup', 0);
            n.focusNode(box);
        ";

    #[gpui::test]
    fn set_modifiers_reports_the_held_modifier_keys(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, REPEATED);

        h.set_modifiers(Modifiers::shift());

        assert_eq!(log(&h), "keydown:Shift:false");
    }

    #[gpui::test]
    fn set_modifiers_none_releases_the_held_keys(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, REPEATED);

        h.set_modifiers(Modifiers::shift());
        h.set_modifiers(Modifiers::none());

        assert_eq!(log(&h), "keydown:Shift:false,keyup:Shift:false");
    }

    /// A `box` with `focus` and `blur` listeners on the window that log their type.
    const WINDOWED: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.appendChild(n.rootNodeId(), box);
            globalThis.log = [];
            globalThis.cb = {
                0: (e) => {
                    globalThis.log.push(e.type);
                },
            };
            listen(n.windowTargetId(), 'focus', 0);
            listen(n.windowTargetId(), 'blur', 0);
        ";

    /// A focused `box` with `focus` and `focusin` listeners that log `type`.
    const FOCUSED: &str = r"
            const n = __inca_native__;
            const box = n.createNode('div');
            n.setStyle(box, 'width', 100);
            n.setStyle(box, 'height', 50);
            n.setAttribute(box, 'tabindex', '0');
            n.appendChild(n.rootNodeId(), box);
            globalThis.log = [];
            globalThis.cb = {
                0: (e) => {
                    globalThis.log.push(e.type);
                },
            };
            listen(box, 'focus', 0);
            listen(box, 'focusin', 0);
            n.focusNode(box);
        ";

    #[gpui::test]
    fn activate_window_refocuses_the_focused_node(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, FOCUSED);
        assert_eq!(log(&h), "focus,focusin");

        h.activate_window();

        assert_eq!(log(&h), "focus,focusin,focus,focusin");
    }

    #[gpui::test]
    fn key_repeat_marks_each_key_as_a_repeat(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, REPEATED);

        h.key_repeat("a");

        assert_eq!(log(&h), "keydown:a:true");
    }

    #[gpui::test]
    #[should_panic(expected = "InvalidKeystrokeError")]
    fn key_repeat_panics_on_an_unparseable_key(cx: &mut TestAppContext) {
        Harness::load(cx, TEST_ENTRY_PATH, "").key_repeat("a-b");
    }

    #[gpui::test]
    fn key_repeat_repeats_each_key_in_order(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, REPEATED);

        h.key_repeat("a b");

        assert_eq!(log(&h), "keydown:a:true,keydown:b:true");
    }

    #[gpui::test]
    fn keystrokes_press_each_key_in_order_and_release_none(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, KEYED);

        h.keystrokes("a cmd-b x");

        assert_eq!(log(&h), "keydown:a:false,keydown:b:true,keydown:x:false");
    }

    #[gpui::test]
    fn key_up_releases_each_key_in_order(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, KEYED);

        h.key_up("a cmd-b x");

        assert_eq!(log(&h), "keyup:a:false,keyup:b:true,keyup:x:false");
    }

    #[gpui::test]
    fn activate_window_reports_the_window_active(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, WINDOWED);

        h.activate_window();

        assert_eq!(log(&h), "focus");
    }

    #[gpui::test]
    fn deactivate_window_reports_the_window_inactive(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, WINDOWED);

        h.activate_window();
        h.deactivate_window();

        assert_eq!(log(&h), "focus,blur");
    }

    #[gpui::test]
    #[should_panic(expected = "the window is not active")]
    fn deactivate_window_panics_on_an_inactive_window(cx: &mut TestAppContext) {
        Harness::load(cx, TEST_ENTRY_PATH, "").deactivate_window();
    }

    #[gpui::test]
    #[should_panic(expected = "the window is already active")]
    fn activate_window_panics_on_an_active_window(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, "");
        h.activate_window();
        h.activate_window();
    }

    #[gpui::test]
    fn activate_window_reports_no_modifier_key_held(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, REPEATED);

        h.set_modifiers(Modifiers::shift());
        h.activate_window();
        h.set_modifiers(Modifiers::none());

        assert_eq!(log(&h), "keydown:Shift:false");
    }

    #[gpui::test]
    fn deactivate_window_reports_no_modifier_key_held(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, REPEATED);

        h.activate_window();
        h.set_modifiers(Modifiers::shift());
        h.deactivate_window();
        h.set_modifiers(Modifiers::none());

        assert_eq!(log(&h), "keydown:Shift:false");
    }

    #[gpui::test]
    #[should_panic(expected = "the window is open")]
    fn settle_panics_once_the_window_has_closed(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, "");
        h.cx.update(|window, _| window.remove_window());
        h.settle();
    }

    #[gpui::test]
    fn advance_clock_runs_a_due_timer_before_it_returns(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            TEST_ENTRY_PATH,
            r"
            globalThis.ticks = 0;
            setTimeout(() => {
                globalThis.ticks += 1;
            }, 100);
            ",
        );

        h.advance_clock(Duration::from_millis(99));
        assert_eq!(eval_f64(&h, "globalThis.ticks"), 0.0);
        h.advance_clock(Duration::from_millis(1));
        assert_eq!(eval_f64(&h, "globalThis.ticks"), 1.0);
    }

    #[gpui::test]
    fn settle_runs_a_pending_frame_callback_once(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            TEST_ENTRY_PATH,
            r"
            globalThis.frames = 0;
            requestAnimationFrame(() => {
                globalThis.frames += 1;
            });
            ",
        );

        h.settle();

        assert_eq!(eval_f64(&h, "globalThis.frames"), 1.0);
    }

    #[gpui::test]
    fn settle_leaves_a_nested_frame_request_to_the_next_settle(cx: &mut TestAppContext) {
        let mut h = Harness::load(
            cx,
            TEST_ENTRY_PATH,
            r"
            globalThis.frames = 0;
            requestAnimationFrame(() => {
                requestAnimationFrame(() => {
                    globalThis.frames += 1;
                });
            });
            ",
        );
        assert_eq!(eval_f64(&h, "globalThis.frames"), 0.0);

        h.settle();

        assert_eq!(eval_f64(&h, "globalThis.frames"), 1.0);
        h.settle();
        assert_eq!(eval_f64(&h, "globalThis.frames"), 1.0);
    }

    const BARRED: &str = r"
            const n = __inca_native__;
            const c = n.createNode('div');
            n.setStyle(c, 'width', 100);
            n.setStyle(c, 'height', 50);
            n.setStyle(c, 'overflow_y', 'scroll');
            n.appendChild(n.rootNodeId(), c);
            const inner = n.createNode('div');
            n.setStyle(inner, 'width', 50);
            n.setStyle(inner, 'height', 300);
            n.appendChild(c, inner);
            globalThis.bar = c;
        ";

    fn client(h: &Harness) -> f64 {
        eval_f64(
            h,
            "__inca_native__.nodeHandle(globalThis.bar)[0].clientWidth",
        )
    }

    #[gpui::test]
    fn scrollbars_overlay_reserves_no_strip(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, BARRED);

        h.scrollbars(Scrollbars::Overlay);

        assert_eq!(client(&h), 100.0);
    }

    #[gpui::test]
    fn scrollbars_classic_after_overlay_reserves_the_strip(cx: &mut TestAppContext) {
        let mut h = Harness::load(cx, TEST_ENTRY_PATH, BARRED);
        h.scrollbars(Scrollbars::Overlay);

        h.scrollbars(Scrollbars::Classic);

        assert_eq!(client(&h), 85.0);
    }
}
