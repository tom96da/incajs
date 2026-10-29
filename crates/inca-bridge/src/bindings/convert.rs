// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use rquickjs::function::Opt;
use rquickjs::{Ctx, Exception, FromJs, IntoJs, Result as JsResult, Value};

use inca_gpui::element::scroll::Align;
use inca_gpui::{NodeId, TreeError, VirtualNode, VirtualTree};

use crate::EventTargetId;
use crate::event_target::{DOCUMENT_TAG, WINDOW_TAG};

/// An optional boolean argument: omitted, `undefined` and `null` give the
/// default.
pub(super) type Flag = Opt<Option<bool>>;

/// A whole number in `u32` range. `noun` names the argument in the message.
/// `NaN`, `1.5`, `-1`, `Infinity`, `2**32` and a non-number throw a
/// `TypeError`.
fn whole_u32(ctx: &Ctx<'_>, value: &Value<'_>, noun: &str) -> JsResult<u32> {
    let number = value
        .as_number()
        .ok_or_else(|| Exception::throw_type(ctx, &format!("{noun} must be a number")))?;
    if number.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&number) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        return Ok(number as u32);
    }
    Err(Exception::throw_type(
        ctx,
        &format!("{noun} must be a whole number in the range of an unsigned 32-bit integer"),
    ))
}

/// A node id argument. Accepts a finite whole number in `u32` range;
/// anything else (`NaN`, `1.5`, `-1`, `Infinity`, `2**32`, a non-number)
/// throws a `TypeError`.
#[derive(Debug, Clone, Copy)]
pub(super) struct JsNodeId(pub(super) NodeId);

impl<'js> FromJs<'js> for JsNodeId {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        whole_u32(ctx, &value, "node id").map(Self)
    }
}

/// A callback id argument, checked like [`JsNodeId`].
#[derive(Debug, Clone, Copy)]
pub(super) struct JsCallbackId(pub(super) u32);

impl<'js> FromJs<'js> for JsCallbackId {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        whole_u32(ctx, &value, "callback id").map(Self)
    }
}

/// A timer id argument, checked like [`JsNodeId`].
#[derive(Debug, Clone, Copy)]
pub(super) struct JsTimerId(pub(super) u32);

impl<'js> FromJs<'js> for JsTimerId {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        whole_u32(ctx, &value, "timer id").map(Self)
    }
}

/// A delay argument in milliseconds, checked like [`JsNodeId`].
#[derive(Debug, Clone, Copy)]
pub(super) struct JsDelay(pub(super) u32);

impl<'js> FromJs<'js> for JsDelay {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        whole_u32(ctx, &value, "delay").map(Self)
    }
}

/// A scroll alignment argument: `"start"`, `"center"`, `"end"` or `"nearest"`.
/// Any other string throws a `TypeError`.
#[derive(Debug, Clone, Copy)]
pub(super) struct JsAlign(pub(super) Align);

impl<'js> FromJs<'js> for JsAlign {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        let name = String::from_js(ctx, value)?;
        match name.as_str() {
            "start" => Ok(Self(Align::Start)),
            "center" => Ok(Self(Align::Center)),
            "end" => Ok(Self(Align::End)),
            "nearest" => Ok(Self(Align::Nearest)),
            _ => Err(Exception::throw_type(
                ctx,
                &format!("unknown scroll alignment: {name}"),
            )),
        }
    }
}

/// An event target argument: a node id as [`JsNodeId`] takes it, or the tag
/// `"$document"` or `"$window"`. Any other string throws a `TypeError`.
#[derive(Debug, Clone, Copy)]
pub(super) struct JsEventTargetId(pub(super) EventTargetId);

impl<'js> FromJs<'js> for JsEventTargetId {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        let Some(tag) = value.as_string() else {
            return JsNodeId::from_js(ctx, value).map(|JsNodeId(id)| Self(EventTargetId::Node(id)));
        };
        match tag.to_string()?.as_str() {
            DOCUMENT_TAG => Ok(Self(EventTargetId::Document)),
            WINDOW_TAG => Ok(Self(EventTargetId::Window)),
            other => Err(Exception::throw_type(
                ctx,
                &format!(
                    concat!(
                        "unknown event target tag: {other}. ",
                        "pass a node id, \"{document}\" or \"{window}\""
                    ),
                    other = other,
                    document = DOCUMENT_TAG,
                    window = WINDOW_TAG,
                ),
            )),
        }
    }
}

/// `value` as a JS value, or `null` for `None`.
pub(crate) fn or_null<'js, T: IntoJs<'js>>(
    ctx: &Ctx<'js>,
    value: Option<T>,
) -> JsResult<Value<'js>> {
    match value {
        Some(value) => value.into_js(ctx),
        None => Ok(Value::new_null(ctx.clone())),
    }
}

/// The node `id` of `tree`, or a thrown `TypeError` when it does not exist.
pub(super) fn existing_node<'a>(
    ctx: &Ctx<'_>,
    tree: &'a VirtualTree,
    id: NodeId,
) -> JsResult<&'a VirtualNode> {
    tree.get(id)
        .ok_or_else(|| throw_tree_error(ctx, TreeError::NodeNotFound(id)))
}

pub(super) fn throw_tree_error(ctx: &Ctx<'_>, err: TreeError) -> rquickjs::Error {
    match err {
        TreeError::WouldCycle(_) => {
            Exception::throw_dom(ctx, "HierarchyRequestError", &err.to_string())
        }
        _ => Exception::throw_type(ctx, &err.to_string()),
    }
}
