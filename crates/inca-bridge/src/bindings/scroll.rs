// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use gpui::px;
use rquickjs::{Ctx, Exception, Function, IntoJs, Object, Result as JsResult, Value};

use inca_gpui::{TreeError, is_scroll_container};

use inca_gpui::element::scroll::Align;

use super::convert::{JsNodeId, throw_tree_error};
use super::host::Host;

fn install_scroll_into_view<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    native: &Object<'js>,
) -> JsResult<()> {
    let host = Rc::clone(host);
    native.set(
        "scrollIntoViewNode",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>,
                  JsNodeId(node_id): JsNodeId,
                  block: String,
                  inline: String|
                  -> JsResult<()> {
                let align = |name: &str| match name {
                    "start" => Ok(Align::Start),
                    "center" => Ok(Align::Center),
                    "end" => Ok(Align::End),
                    "nearest" => Ok(Align::Nearest),
                    _ => Err(Exception::throw_type(
                        &ctx,
                        &format!("unknown scroll alignment: {name}"),
                    )),
                };
                let (block, inline) = (align(&block)?, align(&inline)?);
                let mut host = host.borrow_mut();
                if host.tree.get(node_id).is_none() {
                    return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                }
                if !host.reveal(node_id, block, inline) {
                    host.scroll_fresh.push((node_id, block, inline));
                }
                Ok(())
            },
        )?,
    )?;

    Ok(())
}

pub(super) fn install_scroll<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    native: &Object<'js>,
) -> JsResult<()> {
    {
        let host = Rc::clone(host);
        native.set(
            "scrollOf",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, JsNodeId(node_id): JsNodeId| -> JsResult<Value<'js>> {
                    let host = host.borrow();
                    let Some(node) = host.tree.get(node_id) else {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    };
                    if !is_scroll_container(node) {
                        return Ok(Value::new_null(ctx.clone()));
                    }
                    let handle = host.scroll_handle(node_id);
                    let (offset, max) = (handle.offset(), handle.max_offset());
                    // gpui clamps on the next draw; report the same value.
                    [
                        -offset.x.clamp(-max.x, px(0.)),
                        -offset.y.clamp(-max.y, px(0.)),
                        max.x,
                        max.y,
                    ]
                    .map(|px| f64::from(f32::from(px)))
                    .into_js(&ctx)
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "scrollNode",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      JsNodeId(node_id): JsNodeId,
                      left: Option<f64>,
                      top: Option<f64>|
                      -> JsResult<()> {
                    let host = host.borrow();
                    let Some(node) = host.tree.get(node_id) else {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    };
                    if !is_scroll_container(node) {
                        return Ok(());
                    }
                    let handle = host.scroll_handle(node_id);
                    let mut offset = handle.offset();
                    // Narrowing is fine for pixel offsets; the upper clamp is gpui's.
                    #[allow(clippy::cast_possible_truncation)]
                    let negated = |v: f64| px(-(v.max(0.0) as f32));
                    if let Some(left) = left {
                        offset.x = negated(left);
                    }
                    if let Some(top) = top {
                        offset.y = negated(top);
                    }
                    let (before, max) = (handle.offset(), handle.max_offset());
                    let clamped = |v: gpui::Pixels, max: gpui::Pixels| {
                        -f32::from(v).clamp(-f32::from(max), 0.0)
                    };
                    handle.set_offset(offset);
                    // Recorded boxes follow the new offset, so a reveal in the same tick
                    // starts from where the content now is.
                    host.shift_descendants(
                        node_id,
                        clamped(offset.x, max.x) - clamped(before.x, max.x),
                        clamped(offset.y, max.y) - clamped(before.y, max.y),
                    );
                    Ok(())
                },
            )?,
        )?;
    }

    install_scroll_into_view(ctx, host, native)?;

    Ok(())
}
