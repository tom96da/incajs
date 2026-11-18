// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The members that act on an element: `focus`, `blur` and `click`.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, FromJs, Object, Result as JsResult, Value};

use inca_jsenv::required_prototype;

use super::super::method;
use super::scroll::option_of;
use crate::bindings::handle::{HTML_ELEMENT, HTML_OR_SVG};
use crate::bindings::host::Host;

/// Whether the first argument of `focus` is an object whose `preventScroll` is true.
fn prevent_scroll<'js>(ctx: &Ctx<'js>, args: &[Value<'js>]) -> JsResult<bool> {
    let value = option_of(ctx, args.first(), "preventScroll")?;
    Ok(Coerced::<bool>::from_js(ctx, value)?.0)
}

/// Defines `focus` and `blur` on `proto`.
fn define_focus<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Rc<RefCell<Host>>,
) -> JsResult<()> {
    method(
        ctx,
        proto,
        host,
        "focus",
        0,
        &HTML_OR_SVG,
        |ctx, host, id, _, args| {
            let prevent = prevent_scroll(ctx, args)?;
            let mut host = host.borrow_mut();
            if host.tree.is_connected(id) {
                host.focus.request_focus(id, !prevent);
            }
            Ok(())
        },
    )?;
    method(
        ctx,
        proto,
        host,
        "blur",
        0,
        &HTML_OR_SVG,
        |_, host, id, _, _| {
            let mut host = host.borrow_mut();
            if host.tree.get(id).is_some() {
                host.focus.request_blur(id);
            }
            Ok(())
        },
    )
}

/// Defines `click` on `proto`, the prototype of `HTMLElement`, and `focus` and `blur` on it and on
/// the foreign element.
pub(super) fn install<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Rc<RefCell<Host>>,
) -> JsResult<()> {
    method(
        ctx,
        proto,
        host,
        "click",
        0,
        &HTML_ELEMENT,
        |ctx, host, id, _, _| {
            if host.borrow().tree.get(id).is_some() {
                crate::event::click(ctx, host, id);
            }
            Ok(())
        },
    )?;
    define_focus(ctx, proto, host)?;
    define_focus(ctx, &required_prototype(ctx, "ForeignElement")?, host)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{AppContext as _, TestAppContext};
    use inca_jsenv::Engine;
    use inca_test_macros::gpui_cases;
    use rstest::rstest;

    use crate::bindings::host::Host;
    use crate::test_support::{engine_for, engine_with_bindings, mount_active};

    /// An engine with the global `el`, a `div` the tree has and that has no parent.
    fn with_loose_node() -> (Engine, Rc<RefCell<Host>>) {
        let (engine, host) = engine_with_bindings();
        engine
            .eval::<()>(
                r"
                globalThis.id = __inca_native__.createNode('div');
                globalThis.el = __inca_native__.nodeHandle(id)[0];
                ",
            )
            .unwrap();
        (engine, host)
    }

    #[test]
    fn blur_of_a_node_the_tree_has_queues_a_request() {
        let (engine, host) = with_loose_node();
        assert!(!host.borrow().focus.has_queued_work());

        engine.eval::<()>("el.blur()").unwrap();

        assert!(host.borrow().focus.has_queued_work());
    }

    #[test]
    fn blur_of_a_freed_node_queues_nothing() {
        let (engine, host) = with_loose_node();
        engine
            .eval::<()>("__inca_native__.destroyNode(id)")
            .unwrap();

        engine.eval::<()>("el.blur()").unwrap();

        assert!(!host.borrow().focus.has_queued_work());
    }

    #[rstest]
    #[case::detached("undefined")]
    #[case::freed("__inca_native__.destroyNode(id)")]
    fn focus_of_a_node_that_is_not_connected_queues_nothing(#[case] setup: &str) {
        let (engine, host) = with_loose_node();
        engine.eval::<()>(setup).unwrap();

        engine.eval::<()>("el.focus()").unwrap();

        assert!(!host.borrow().focus.has_queued_work());
    }

    #[test]
    fn click_of_a_never_inserted_element_runs_its_listener() {
        let (engine, _host) = engine_with_bindings();

        engine
            .eval::<()>(
                r"
                const n = __inca_native__;
                const id = n.createNode('div');
                globalThis.clicks = 0;
                n.addEventListener(
                    id,
                    'click',
                    () => {
                        clicks++;
                    },
                    false,
                    false,
                );
                n.nodeHandle(id)[0].click();
                ",
            )
            .unwrap();

        assert_eq!(engine.eval::<u32>("clicks").unwrap(), 1);
    }

    /// Draws the window once more, the frame that applies a focus request.
    fn draw(cx: &mut TestAppContext, window: gpui::WindowHandle<crate::test_support::TestRoot>) {
        cx.update_window(window.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })
        .unwrap();
    }

    /// Mounts a scroll container 100 high with ten focusable children 50 high under the root and
    /// runs `call`, whose `el` is the seventh child, then draws the frame that applies it. Returns
    /// the vertical scroll offset of the container and whether `el` is focused.
    fn scroll_after(cx: &mut TestAppContext, call: &str) -> (f32, bool) {
        let host = Rc::new(RefCell::new(Host::default()));
        let engine = engine_for(&host);
        let container: u32 = engine
            .eval(
                r"
                const n = __inca_native__;
                const box = n.createNode('div');
                n.setStyle(box, 'height', 100);
                n.setStyle(box, 'overflow_y', 'scroll');
                n.appendChild(n.rootNodeId(), box);
                globalThis.ids = [];
                for (let i = 0; i < 10; i++) {
                    const item = n.createNode('div');
                    n.setStyle(item, 'height', 50);
                    n.setAttribute(item, 'tabindex', '0');
                    n.appendChild(box, item);
                    ids.push(item);
                }
                globalThis.el = n.nodeHandle(ids[6])[0];
                box;
                ",
            )
            .unwrap();
        let root = host.borrow().root();
        let window = mount_active(cx, &host, &engine, root);

        engine.eval::<()>(call).unwrap();
        draw(cx, window);

        let offset = host.borrow().scroll_handle(container).offset();
        let focused = engine
            .eval::<bool>("__inca_native__.focusedNode() === ids[6]")
            .unwrap();
        (f32::from(offset.y), focused)
    }

    // The child spans 300 to 350 in a container 100 high, so its center is revealed at 275.
    #[gpui_cases]
    #[gpui::test]
    #[case::no_argument("el.focus()")]
    #[case::a_primitive("el.focus(true)")]
    fn focus_scrolls_the_node_into_view(cx: &mut TestAppContext, call: &str) {
        assert_eq!(scroll_after(cx, call), (-275.0, true));
    }

    #[gpui::test]
    fn focus_that_prevents_scroll_keeps_the_offset(cx: &mut TestAppContext) {
        assert_eq!(
            scroll_after(cx, "el.focus({ preventScroll: true })"),
            (0.0, true)
        );
    }
}
