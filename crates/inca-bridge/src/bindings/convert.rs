// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use rquickjs::{Ctx, Exception, FromJs, Result as JsResult, Value};

use inca_gpui::{NodeId, TreeError};

/// A node id argument. Accepts a finite whole number in `u32` range;
/// anything else (`NaN`, `1.5`, `-1`, `Infinity`, `2**32`, a non-number)
/// throws a `TypeError`.
#[derive(Debug, Clone, Copy)]
pub(super) struct JsNodeId(pub(super) NodeId);

impl<'js> FromJs<'js> for JsNodeId {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        let number = value
            .as_number()
            .ok_or_else(|| Exception::throw_type(ctx, "node id must be a number"))?;
        if number.fract() == 0.0 && (0.0..=f64::from(NodeId::MAX)).contains(&number) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            return Ok(Self(number as NodeId));
        }
        Err(Exception::throw_type(
            ctx,
            "node id must be a whole number in the range of an unsigned 32-bit integer",
        ))
    }
}

pub(super) fn throw_tree_error(ctx: &Ctx<'_>, err: TreeError) -> rquickjs::Error {
    Exception::throw_type(ctx, &err.to_string())
}
