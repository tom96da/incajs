// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use rquickjs::convert::Coerced;
use rquickjs::function::Opt;
use rquickjs::{Class, Ctx, Exception, FromJs, IntoJs, Result as JsResult, Value};

use inca_document::geometry::Align;
use inca_node::{NodeId, TreeError, VirtualNode, VirtualTree};

use crate::abort::IncaSignal;
use crate::event::WINDOW_TAG;
use inca_document::EventTargetId;

/// An optional boolean argument: omitted, `undefined` and `null` give the
/// default.
pub(crate) type Flag = Opt<Option<bool>>;

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

/// An offset, a count or an index: the argument converts to an unsigned 32-bit integer.
pub(crate) fn unsigned(Coerced(value): Coerced<i32>) -> u32 {
    value.cast_unsigned()
}

/// A length or an index as the `u32` JS reads, saturating at `u32::MAX`.
pub(crate) fn saturated(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// A node id argument. Accepts a finite whole number in `u32` range;
/// anything else (`NaN`, `1.5`, `-1`, `Infinity`, `2**32`, a non-number)
/// throws a `TypeError`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct JsNodeId(pub(crate) NodeId);

impl<'js> FromJs<'js> for JsNodeId {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        whole_u32(ctx, &value, "node id").map(Self)
    }
}

/// A callback id argument, checked like [`JsNodeId`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct JsCallbackId(pub(crate) u32);

impl<'js> FromJs<'js> for JsCallbackId {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        whole_u32(ctx, &value, "callback id").map(Self)
    }
}

/// A scroll alignment argument: `"start"`, `"center"`, `"end"` or `"nearest"`.
/// Any other string throws a `TypeError`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct JsAlign(pub(crate) Align);

impl<'js> FromJs<'js> for JsAlign {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        let name = String::from_js(ctx, value)?;
        Align::from_name(&name)
            .map(Self)
            .ok_or_else(|| Exception::throw_type(ctx, &format!("unknown scroll alignment: {name}")))
    }
}

/// An event target argument: a node id as [`JsNodeId`] takes it, the tag `"$window"`, or an
/// `AbortSignal` object. Any other string throws a `TypeError`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct JsEventTargetId(pub(crate) EventTargetId);

impl<'js> FromJs<'js> for JsEventTargetId {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        let Some(tag) = value.as_string() else {
            if let Ok(signal) = Class::<IncaSignal>::from_value(&value) {
                return Ok(Self(EventTargetId::Signal(signal.borrow().id())));
            }
            return JsNodeId::from_js(ctx, value).map(|JsNodeId(id)| Self(EventTargetId::Node(id)));
        };
        match tag.to_string()?.as_str() {
            WINDOW_TAG => Ok(Self(EventTargetId::Window)),
            other => Err(Exception::throw_type(
                ctx,
                &format!(
                    concat!(
                        "unknown event target tag: {other}. ",
                        "pass a node id, \"{window}\" or an AbortSignal"
                    ),
                    other = other,
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
pub(crate) fn existing_node<'a>(
    ctx: &Ctx<'_>,
    tree: &'a VirtualTree,
    id: NodeId,
) -> JsResult<&'a VirtualNode> {
    tree.get(id)
        .ok_or_else(|| throw_tree_error(ctx, TreeError::NodeNotFound(id)))
}

pub(crate) fn throw_tree_error(ctx: &Ctx<'_>, err: TreeError) -> rquickjs::Error {
    match err {
        TreeError::WouldCycle(_)
        | TreeError::LeafParent(_)
        | TreeError::DocumentInserted(_)
        | TreeError::DocumentChild(_) => {
            Exception::throw_dom(ctx, "HierarchyRequestError", &err.to_string())
        }
        TreeError::NotAChild { .. } => Exception::throw_dom(ctx, "NotFoundError", &err.to_string()),
        TreeError::NoParent(_) | TreeError::DocumentParent(_) => {
            Exception::throw_dom(ctx, "NoModificationAllowedError", &err.to_string())
        }
        TreeError::IndexSize => Exception::throw_dom(ctx, "IndexSizeError", &err.to_string()),
        _ => Exception::throw_type(ctx, &err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::test_support::{engine_with_bindings, error_name};

    #[test]
    fn a_signal_object_is_an_event_target() {
        let (engine, _host) = engine_with_bindings();

        let removed: bool = engine
            .eval("__inca_native__.removeEventListener(new AbortController().signal, 'abort', 1)")
            .unwrap();

        assert!(!removed);
    }

    #[rstest]
    #[case::plain("{}")]
    #[case::forged("Object.create(AbortSignal.prototype)")]
    #[case::controller("new AbortController()")]
    fn another_object_is_no_event_target(#[case] target: &str) {
        let (engine, _host) = engine_with_bindings();

        let name = error_name(
            &engine,
            &format!("__inca_native__.removeEventListener({target}, 'abort', 1)"),
        );

        assert_eq!(name, "TypeError");
    }

    #[test]
    fn an_unknown_tag_names_the_three_forms() {
        let (engine, _host) = engine_with_bindings();

        let message: String = engine
            .eval(
                r"
                try { __inca_native__.removeEventListener('nope', 'abort', 1); }
                catch (e) { e.message; }
                ",
            )
            .unwrap();

        assert_eq!(
            message,
            "unknown event target tag: nope. pass a node id, \"$window\" or an AbortSignal"
        );
    }

    #[rstest]
    #[case::nan("NaN")]
    #[case::fractional("1.5")]
    #[case::negative("-1")]
    #[case::infinity("Infinity")]
    #[case::above_u32("2 ** 32")]
    #[case::numeric_string("'1'")]
    fn node_id_that_is_not_a_whole_u32_throws_type_error(#[case] bad: &str) {
        let (engine, _host) = engine_with_bindings();

        let script = format!(
            r"
            (() => {{ const node = __inca_native__.createNode('div');
            const rejects = (f) => {{
                try {{ f(); return false; }} catch (e) {{ return e instanceof TypeError; }}
            }};
            return [
                rejects(() => __inca_native__.setStyle({bad}, 'width', 1)),
                rejects(() => __inca_native__.removeStyle({bad}, 'width')),
                rejects(() => __inca_native__.getAttribute({bad}, 'label')),
                rejects(() => __inca_native__.removeAttribute({bad}, 'label')),
                rejects(() => __inca_native__.appendChild({bad}, node)),
                rejects(() => __inca_native__.appendChild(node, {bad})),
                rejects(() => __inca_native__.insertBefore(node, node, {bad})),
                rejects(() => __inca_native__.destroyNode({bad})),
                rejects(() => __inca_native__.holdNode({bad})),
                rejects(() => __inca_native__.releaseNode({bad})),
                rejects(() => __inca_native__.addEventListener({bad}, 'click', () => {{}})),
                rejects(() => __inca_native__.removeEventListener({bad}, 'click', 1)),
                rejects(() => __inca_native__.focusNode({bad})),
                rejects(() => __inca_native__.dispatchEvent({bad}, {{}})),
                rejects(() => __inca_native__.click({bad})),
            ].every(Boolean); }})()
            "
        );
        assert!(engine.eval::<bool>(&script).unwrap(), "{bad}");
    }
}
