// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The members that act on an element: `focus`, `blur` and `click`.

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
fn define_focus<'js>(ctx: &Ctx<'js>, proto: &Object<'js>, host: &Rc<Host>) -> JsResult<()> {
    method(
        ctx,
        proto,
        host,
        "focus",
        0,
        &HTML_OR_SVG,
        |ctx, host, id, _, args| {
            let prevent = prevent_scroll(ctx, args)?;
            let mut host = host.document.borrow_mut();
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
            let mut host = host.document.borrow_mut();
            if host.tree.get(id).is_some() {
                host.focus.request_blur(id);
            }
            Ok(())
        },
    )
}

/// Defines `click` on `proto`, the prototype of `HTMLElement`, and `focus` and `blur` on it and on
/// the foreign element.
pub(super) fn install<'js>(ctx: &Ctx<'js>, proto: &Object<'js>, host: &Rc<Host>) -> JsResult<()> {
    method(
        ctx,
        proto,
        host,
        "click",
        0,
        &HTML_ELEMENT,
        |ctx, host, id, _, _| {
            if host.document.borrow().tree.get(id).is_some() {
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

    use std::rc::Rc;

    use inca_jsenv::Engine;
    use rstest::rstest;

    use crate::bindings::host::Host;
    use crate::test_support::engine_with_bindings;

    /// An engine with the global `el`, a `div` the tree has and that has no parent.
    fn with_loose_node() -> (Engine, Rc<Host>) {
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
        assert!(!host.document.borrow().focus.has_queued_work());

        engine.eval::<()>("el.blur()").unwrap();

        assert!(host.document.borrow().focus.has_queued_work());
    }

    #[test]
    fn blur_of_a_freed_node_queues_nothing() {
        let (engine, host) = with_loose_node();
        engine
            .eval::<()>("__inca_native__.destroyNode(id)")
            .unwrap();

        engine.eval::<()>("el.blur()").unwrap();

        assert!(!host.document.borrow().focus.has_queued_work());
    }

    #[rstest]
    #[case::detached("undefined")]
    #[case::freed("__inca_native__.destroyNode(id)")]
    fn focus_of_a_node_that_is_not_connected_queues_nothing(#[case] setup: &str) {
        let (engine, host) = with_loose_node();
        engine.eval::<()>(setup).unwrap();

        engine.eval::<()>("el.focus()").unwrap();

        assert!(!host.document.borrow().focus.has_queued_work());
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
}
