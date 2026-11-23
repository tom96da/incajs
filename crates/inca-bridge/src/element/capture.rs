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
    if !request_capture(host, node) {
        return Err(Exception::throw_dom(
            ctx,
            "InvalidStateError",
            "the node is not in the mounted tree. mount it before capturing the pointer",
        ));
    }
    Ok(())
}

/// Asks for the capture of the mouse pointer for `node`. Returns whether `node` is connected.
fn request_capture(host: &Host, node: NodeId) -> bool {
    let held = host.window.borrow().held_buttons() != 0;
    let mut document = host.document.borrow_mut();
    let connected = document.tree().is_connected(node);
    if held && connected {
        document.capture_pending = Some(node);
    }
    connected
}

/// Whether `node` has the capture of the pointer.
fn has_capture(host: &DocumentState, node: NodeId, pointer_id: i32) -> bool {
    pointer_id == MOUSE_POINTER
        && host.capture_pending == Some(node)
        && host.tree().is_connected(node)
}

/// Releases the capture of the pointer when `node` has it.
fn release_capture(ctx: &Ctx<'_>, host: &Rc<Host>, node: NodeId, pointer_id: i32) -> JsResult<()> {
    known_pointer(ctx, pointer_id)?;
    drop_capture(host, node);
    Ok(())
}

/// Drops the pending capture when `node` has it.
fn drop_capture(host: &Host, node: NodeId) {
    let mut document = host.document.borrow_mut();
    if document.capture_pending == Some(node) {
        document.capture_pending = None;
    }
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

    use std::rc::Rc;

    use rstest::rstest;

    use super::Host;
    use crate::test_support::{engine_with_view, error_name};

    /// A mounted node, and the engine and host it lives in.
    fn mounted(held: u8) -> (inca_jsenv::Engine, Rc<Host>, u32) {
        let (engine, host, view) = engine_with_view();
        view.borrow_mut().held = held;
        let node: u32 = engine
            .eval(
                r"
                globalThis.n = __inca_native__.createNode('div');
                __inca_native__.appendChild(__inca_native__.rootNodeId(), n);
                n;
                ",
            )
            .unwrap();
        (engine, host, node)
    }

    #[rstest]
    #[case::set("setPointerCapture")]
    #[case::release("releasePointerCapture")]
    fn an_unknown_pointer_throws_not_found(#[case] method: &str) {
        let (engine, _host, _node) = mounted(1);
        assert_eq!(
            error_name(&engine, &format!("__inca_native__.{method}(n, 2)")),
            "NotFoundError"
        );
    }

    #[rstest]
    #[case::detached("__inca_native__.createNode('div')")]
    #[case::destroyed("9999")]
    fn set_pointer_capture_throws_for_a_disconnected_node(#[case] node: &str) {
        let (engine, _host, _node) = mounted(1);
        assert_eq!(
            error_name(
                &engine,
                &format!("__inca_native__.setPointerCapture({node}, 1)")
            ),
            "InvalidStateError"
        );
    }

    #[test]
    fn set_pointer_capture_without_pressed_buttons_is_noop() {
        let (engine, host, _node) = mounted(0);
        engine
            .eval::<()>("__inca_native__.setPointerCapture(n, 1)")
            .unwrap();
        assert_eq!(host.document.borrow().capture_pending, None);
    }

    #[test]
    fn set_pointer_capture_succeeds_for_connected_node() {
        let (engine, host, node) = mounted(1);
        engine
            .eval::<()>("__inca_native__.setPointerCapture(n, 1)")
            .unwrap();
        assert_eq!(host.document.borrow().capture_pending, Some(node));
    }

    #[test]
    fn has_pointer_capture_is_true_only_for_the_pending_owner() {
        let (engine, host, _node) = mounted(1);
        let flags: Vec<bool> = engine
            .eval(
                r"
                const other = __inca_native__.createNode('div');
                __inca_native__.appendChild(__inca_native__.rootNodeId(), other);
                __inca_native__.setPointerCapture(n, 1);
                const owner = __inca_native__.hasPointerCapture(n, 1);
                const stranger = __inca_native__.hasPointerCapture(other, 1);
                const unsupported = __inca_native__.hasPointerCapture(n, 2);
                __inca_native__.releasePointerCapture(other, 1);
                const kept = __inca_native__.hasPointerCapture(n, 1);
                __inca_native__.releasePointerCapture(n, 1);
                [owner, stranger, unsupported, kept, __inca_native__.hasPointerCapture(n, 1)];
                ",
            )
            .unwrap();
        assert_eq!(flags, [true, false, false, true, false]);
        assert_eq!(host.document.borrow().capture_pending, None);
    }

    #[test]
    fn a_removed_pending_owner_no_longer_has_the_capture() {
        let (engine, _host, _node) = mounted(1);
        let has: bool = engine
            .eval(
                r"
                __inca_native__.setPointerCapture(n, 1);
                __inca_native__.removeChild(__inca_native__.rootNodeId(), n);
                __inca_native__.hasPointerCapture(n, 1);
                ",
            )
            .unwrap();
        assert!(!has);
    }
}
