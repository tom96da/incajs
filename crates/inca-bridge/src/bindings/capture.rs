// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, Exception, Result as JsResult};

use super::Natives;
use super::convert::JsNodeId;
use super::host::Host;

/// The id of the one pointer the host has: the mouse.
const MOUSE_POINTER: i32 = 1;

/// Capturing the mouse pointer.
pub(super) fn install_capture(n: &Natives<'_, '_>) -> JsResult<()> {
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

fn set_pointer_capture<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<i32>) -> JsResult<()> {
    move |ctx, JsNodeId(node), Coerced(pointer_id)| {
        known_pointer(&ctx, pointer_id)?;
        let mut host = host.borrow_mut();
        if !host.tree.is_connected(node) {
            return Err(Exception::throw_dom(
                &ctx,
                "InvalidStateError",
                "the node is not in the mounted tree. mount it before capturing the pointer",
            ));
        }
        if host.held_buttons != 0 {
            host.capture_pending = Some(node);
        }
        Ok(())
    }
}

fn has_pointer_capture(host: Rc<RefCell<Host>>) -> impl Fn(JsNodeId, Coerced<i32>) -> bool {
    move |JsNodeId(node), Coerced(pointer_id)| {
        let host = host.borrow();
        pointer_id == MOUSE_POINTER
            && host.capture_pending == Some(node)
            && host.tree.is_connected(node)
    }
}

fn release_pointer_capture<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<i32>) -> JsResult<()> {
    move |ctx, JsNodeId(node), Coerced(pointer_id)| {
        known_pointer(&ctx, pointer_id)?;
        let mut host = host.borrow_mut();
        if host.capture_pending == Some(node) {
            host.capture_pending = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    use std::rc::Rc;

    use rstest::rstest;

    use super::Host;
    use crate::test_support::{engine_with_bindings, error_name};

    /// A mounted node, and the engine and host it lives in.
    fn mounted(held: u8) -> (inca_jsenv::Engine, Rc<RefCell<Host>>, u32) {
        let (engine, host) = engine_with_bindings();
        host.borrow_mut().held_buttons = held;
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
        assert_eq!(host.borrow().capture_pending, None);
    }

    #[test]
    fn set_pointer_capture_succeeds_for_connected_node() {
        let (engine, host, node) = mounted(1);
        engine
            .eval::<()>("__inca_native__.setPointerCapture(n, 1)")
            .unwrap();
        assert_eq!(host.borrow().capture_pending, Some(node));
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
        assert_eq!(host.borrow().capture_pending, None);
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
