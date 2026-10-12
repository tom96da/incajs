// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use gpui::Bounds;
use rquickjs::function::Opt;
use rquickjs::{Ctx, Function, IntoJs, Object, Result as JsResult, Value};

use inca_gpui::TreeError;

use super::convert::{JsNodeId, throw_tree_error};
use super::host::Host;

/// Requesting and releasing focus.
pub(super) fn install_focus<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    native: &Object<'js>,
) -> JsResult<()> {
    {
        let host = Rc::clone(host);
        native.set(
            "focusNode",
            Function::new(
                ctx.clone(),
                move |JsNodeId(node_id): JsNodeId, prevent_scroll: Opt<bool>| {
                    let mut host = host.borrow_mut();
                    // Focusing a node that is gone is a no-op.
                    if host.tree.get(node_id).is_some() {
                        host.focus
                            .request_focus(node_id, prevent_scroll.0 != Some(true));
                    }
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "blurNode",
            Function::new(ctx.clone(), move |JsNodeId(node_id): JsNodeId| {
                host.borrow_mut().focus.request_blur(node_id);
            })?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "focusedNode",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>| -> JsResult<Value<'js>> {
                let host = host.borrow();
                match host.focus.active_node(&host.tree) {
                    Some(node_id) => node_id.into_js(&ctx),
                    None => Ok(Value::new_null(ctx.clone())),
                }
            })?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "layoutOf",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, JsNodeId(node_id): JsNodeId| -> JsResult<Value<'js>> {
                    let host = host.borrow();
                    if host.tree.get(node_id).is_none() {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    }
                    let Some(layout) = host.layout.borrow().get(&node_id).copied() else {
                        return Ok(Value::new_null(ctx.clone()));
                    };
                    let Bounds { origin, size } = layout.bounds;
                    [
                        origin.x,
                        origin.y,
                        size.width,
                        size.height,
                        layout.border,
                        layout.scroll.x,
                        layout.scroll.y,
                    ]
                    .map(|px| f64::from(f32::from(px)))
                    .into_js(&ctx)
                },
            )?,
        )?;
    }

    Ok(())
}
