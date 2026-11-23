// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pointer capture of an element: the natives that set, release and read the capture of the
//! mouse pointer, and the members `setPointerCapture`, `releasePointerCapture` and
//! `hasPointerCapture`.

use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, Exception, FromJs, Result as JsResult, Value};

use inca_node::NodeId;

use crate::bindings::Host;
use crate::bindings::Natives;
use crate::bindings::convert::JsNodeId;
use crate::bindings::members::method;
use crate::node::handle::ELEMENT;
use inca_document::DocumentState;
use inca_jsenv::class::require;

/// The id of the one pointer the host has: the mouse.
const MOUSE_POINTER: i32 = 1;

/// Capturing the mouse pointer.
pub(crate) fn install_capture(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("setPointerCapture", set_pointer_capture)?;
    n.set("releasePointerCapture", release_pointer_capture)?;
    n.set("hasPointerCapture", has_pointer_capture)
}

/// Throws a `NotFoundError` for an id other than the mouse pointer's.
fn known_pointer(ctx: &Ctx<'_>, pointer_id: i32) -> JsResult<()> {
    if pointer_id == MOUSE_POINTER {
        return Ok(());
    }
    Err(Exception::throw_dom(
        ctx,
        "NotFoundError",
        "the pointer id names no active pointer. use pointer id 1, the mouse",
    ))
}

/// Captures the mouse pointer for `node` while a button is held. A node that is not connected
/// throws `InvalidStateError`.
fn set_capture(ctx: &Ctx<'_>, host: &Rc<Host>, node: NodeId, pointer_id: i32) -> JsResult<()> {
    known_pointer(ctx, pointer_id)?;
    let connected = host
        .document
        .borrow_mut()
        .request_capture(&*host.window.borrow(), node);
    if !connected {
        return Err(Exception::throw_dom(
            ctx,
            "InvalidStateError",
            "the node is not in the mounted tree. mount it before capturing the pointer",
        ));
    }
    Ok(())
}

/// Whether `node` has the capture of the pointer.
fn has_capture(host: &DocumentState, node: NodeId, pointer_id: i32) -> bool {
    pointer_id == MOUSE_POINTER && host.has_capture_request(node)
}

/// Releases the capture of the pointer when `node` has it.
fn release_capture(ctx: &Ctx<'_>, host: &Rc<Host>, node: NodeId, pointer_id: i32) -> JsResult<()> {
    known_pointer(ctx, pointer_id)?;
    host.document.borrow_mut().release_capture_request(node);
    Ok(())
}

fn set_pointer_capture<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<i32>) -> JsResult<()> {
    move |ctx, JsNodeId(node), Coerced(pointer_id)| set_capture(&ctx, &host, node, pointer_id)
}

fn has_pointer_capture(host: Rc<Host>) -> impl Fn(JsNodeId, Coerced<i32>) -> bool {
    move |JsNodeId(node), Coerced(pointer_id)| {
        has_capture(&host.document.borrow(), node, pointer_id)
    }
}

fn release_pointer_capture<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<i32>) -> JsResult<()> {
    move |ctx, JsNodeId(node), Coerced(pointer_id)| release_capture(&ctx, &host, node, pointer_id)
}

/// The pointer id argument of a pointer capture member.
fn pointer_id<'js>(ctx: &Ctx<'js>, name: &str, args: &[Value<'js>]) -> JsResult<i32> {
    require(ctx, name, args, 1)?;
    Ok(Coerced::<i32>::from_js(ctx, args[0].clone())?.0)
}

/// Defines the pointer capture members on `proto`.
pub(super) fn pointer_capture<'js>(
    ctx: &Ctx<'js>,
    proto: &rquickjs::Object<'js>,
    host: &Rc<Host>,
) -> JsResult<()> {
    method(
        ctx,
        proto,
        host,
        "setPointerCapture",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            set_capture(ctx, host, id, pointer_id(ctx, "setPointerCapture", args)?)
        },
    )?;
    method(
        ctx,
        proto,
        host,
        "releasePointerCapture",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            release_capture(
                ctx,
                host,
                id,
                pointer_id(ctx, "releasePointerCapture", args)?,
            )
        },
    )?;
    method(
        ctx,
        proto,
        host,
        "hasPointerCapture",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            let pointer = pointer_id(ctx, "hasPointerCapture", args)?;
            Ok(has_capture(&host.document.borrow(), id, pointer))
        },
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::test_support::{engine_with_view, error_name};

    /// A mounted node `n` while the left button is held, and the engine it lives in.
    fn mounted() -> inca_jsenv::Engine {
        let (engine, _host, view) = engine_with_view();
        view.borrow_mut().held = 1;
        engine
            .eval::<()>(
                r"
                globalThis.n = __inca_native__.createNode('div');
                __inca_native__.appendChild(__inca_native__.rootNodeId(), n);
                ",
            )
            .unwrap();
        engine
    }

    #[rstest]
    #[case::set("setPointerCapture")]
    #[case::release("releasePointerCapture")]
    fn an_unknown_pointer_throws_not_found(#[case] method: &str) {
        let engine = mounted();
        assert_eq!(
            error_name(&engine, &format!("__inca_native__.{method}(n, 2)")),
            "NotFoundError"
        );
    }

    #[rstest]
    #[case::detached("__inca_native__.createNode('div')")]
    #[case::destroyed("9999")]
    fn set_pointer_capture_throws_for_a_disconnected_node(#[case] node: &str) {
        let engine = mounted();
        assert_eq!(
            error_name(
                &engine,
                &format!("__inca_native__.setPointerCapture({node}, 1)")
            ),
            "InvalidStateError"
        );
    }

    #[test]
    fn has_pointer_capture_follows_a_set_and_a_release_of_the_mouse_pointer() {
        let engine = mounted();
        let flags: Vec<bool> = engine
            .eval(
                r"
                __inca_native__.setPointerCapture(n, 1);
                const owner = __inca_native__.hasPointerCapture(n, 1);
                const unsupported = __inca_native__.hasPointerCapture(n, 2);
                __inca_native__.releasePointerCapture(n, 1);
                [owner, unsupported, __inca_native__.hasPointerCapture(n, 1)];
                ",
            )
            .unwrap();
        assert_eq!(flags, [true, false, false]);
    }
}
