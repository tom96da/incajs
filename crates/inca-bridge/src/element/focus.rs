// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Focus and the actions of an element: the natives that request, release and read focus, and the
//! members `focus`, `blur` and `click`.

use super::scroll::option_of;
use crate::bindings::Host;
use crate::bindings::Natives;
use crate::bindings::convert::{Flag, JsNodeId, existing_node, or_null};
use crate::bindings::members::method;
use crate::node::handle::{HTML_ELEMENT, HTML_OR_SVG};
use inca_jsenv::required_prototype;
use rquickjs::convert::Coerced;
use rquickjs::{Ctx, FromJs, Object, Result as JsResult, Value};
use std::rc::Rc;

/// Requesting and releasing focus, and reading the focused node and the box of a node.
pub(crate) fn install_focus(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("focusNode", focus_node)?;
    n.set("blurNode", blur_node)?;
    n.set("focusedNode", focused_node)?;
    n.set("layoutOf", super::layout_boxes::layout_of)
}

fn focus_node<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId, Flag) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), prevent_scroll| {
        let mut host = host.document.borrow_mut();
        existing_node(&ctx, host.tree(), node_id)?;
        host.request_focus(node_id, prevent_scroll.0.flatten() != Some(true));
        Ok(())
    }
}

fn blur_node(host: Rc<Host>) -> impl Fn(JsNodeId) {
    move |JsNodeId(node_id)| host.document.borrow_mut().request_blur(node_id)
}

fn focused_node<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>) -> JsResult<Value<'js>> {
    move |ctx| {
        let host = host.document.borrow();
        or_null(&ctx, host.focus().active_node(host.tree(), host.style()))
    }
}

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
            if host.tree().is_connected(id) {
                host.request_focus(id, !prevent);
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
            if host.tree().get(id).is_some() {
                host.request_blur(id);
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
            if host.document.borrow().tree().get(id).is_some() {
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
    use crate::bindings::Host;
    use crate::test_support::engine_with_bindings;
    use inca_jsenv::Engine;
    use inca_node::AttributeValue;
    use rstest::rstest;
    use std::rc::Rc;

    #[test]
    fn only_focus_attributes_and_display_mark_a_node_for_a_focus_check() {
        let (engine, host) = engine_with_bindings();
        let node: u32 = engine
            .eval("globalThis.n = __inca_native__.createNode('div')")
            .unwrap();
        let dirty = |host: &Rc<Host>| host.document.borrow_mut().take_tab_dirty();
        assert!(dirty(&host).is_empty());

        engine
            .eval::<()>(
                r"
                __inca_native__.setAttribute(n, 'label', 'x');
                __inca_native__.removeAttribute(n, 'label');
                __inca_native__.setStyle(n, 'width', 5);
                __inca_native__.removeStyle(n, 'width');
                ",
            )
            .unwrap();
        assert!(dirty(&host).is_empty());

        // A subtree check queues only nodes that have a tabindex.
        for call in [
            "setAttribute(n, 'tabindex', '0')",
            "setAttribute(n, 'tabIndex', 0)",
            "setAttribute(n, 'disabled', true)",
            "removeAttribute(n, 'disabled')",
            "setStyle(n, 'display', 'none')",
            "removeStyle(n, 'display')",
            "setAttribute(n, 'hidden', true)",
            "removeAttribute(n, 'hidden')",
            "setAttribute(n, 'inert', true)",
            "removeAttribute(n, 'inert')",
            "removeAttribute(n, 'TABINDEX')",
        ] {
            engine
                .eval::<()>(&format!("__inca_native__.{call}"))
                .unwrap();
            assert!(dirty(&host).contains(&node), "{call}");
        }
        engine
            .eval::<()>("__inca_native__.setAttribute(n, 'tabIndex', 1)")
            .unwrap();
        let host = host.document.borrow();
        let attrs = host.tree().get(node).unwrap().attributes();
        assert_eq!(attrs.get("tabindex"), Some(&AttributeValue::Number(1.0)));
        assert!(!attrs.contains_key("tabIndex"));
    }

    #[test]
    fn focused_node_follows_focus_node_and_blur_node() {
        let (engine, _host) = engine_with_bindings();

        let flags: Vec<bool> = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                const other = n.createNode('div');
                const plain = n.createNode('div');
                for (const id of [node, other, plain]) {
                    n.appendChild(n.rootNodeId(), id);
                }
                n.setAttribute(node, 'tabindex', 0);
                n.setAttribute(other, 'tabindex', 0);
                const before = n.focusedNode() === null;
                n.focusNode(plain);
                const plainSkipped = n.focusedNode() === null;
                n.focusNode(node);
                const focused = n.focusedNode() === node;
                n.blurNode(other);
                const kept = n.focusedNode() === node;
                n.blurNode(node);
                [before, plainSkipped, focused, kept, n.focusedNode() === null];
                ",
            )
            .unwrap();

        assert_eq!(flags, [true; 5]);
    }

    #[test]
    fn focus_node_on_an_unknown_node_throws_type_error() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.focusNode(999);
                } catch (e) {
                    caught = e instanceof TypeError;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught);
    }

    #[test]
    fn blur_node_ignores_an_unknown_node() {
        let (engine, _host) = engine_with_bindings();

        engine.eval::<()>("__inca_native__.blurNode(999);").unwrap();
    }

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
        assert!(!host.document.borrow().focus().has_queued_work());

        engine.eval::<()>("el.blur()").unwrap();

        assert!(host.document.borrow().focus().has_queued_work());
    }

    #[test]
    fn blur_of_a_freed_node_queues_nothing() {
        let (engine, host) = with_loose_node();
        engine
            .eval::<()>("__inca_native__.destroyNode(id)")
            .unwrap();

        engine.eval::<()>("el.blur()").unwrap();

        assert!(!host.document.borrow().focus().has_queued_work());
    }

    #[rstest]
    #[case::detached("undefined")]
    #[case::freed("__inca_native__.destroyNode(id)")]
    fn focus_of_a_node_that_is_not_connected_queues_nothing(#[case] setup: &str) {
        let (engine, host) = with_loose_node();
        engine.eval::<()>(setup).unwrap();

        engine.eval::<()>("el.focus()").unwrap();

        assert!(!host.document.borrow().focus().has_queued_work());
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
