// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scroll reads and writes of an element and its scroll methods. A node that is not connected
//! reads `0` and ignores a write.

use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, FromJs, Object, Result as JsResult, Value};

use inca_document::geometry::Align;
use inca_node::NodeId;

use super::super::{Shared, accessor, getter, method};
use super::boxes::{client_height, client_width, rounded};
use crate::bindings::handle::ELEMENT;
use crate::bindings::host::Host;
use crate::bindings::scroll::{node_scroll, scroll_into_view, scroll_to};

/// The scroll state of the node `id`, `None` for a node that is not connected or scrolls nothing.
fn scroll_of(host: &Shared, id: NodeId) -> Option<[f64; 4]> {
    let connected = host.document.borrow().tree.is_connected(id);
    connected.then(|| node_scroll(host, id)).flatten()
}

/// The value of the property `name` of an options argument, `undefined` for any other argument.
pub(super) fn option_of<'js>(
    ctx: &Ctx<'js>,
    arg: Option<&Value<'js>>,
    name: &str,
) -> JsResult<Value<'js>> {
    match arg.and_then(|arg| arg.as_object()) {
        Some(options) => options.get(name),
        None => Ok(Value::new_undefined(ctx.clone())),
    }
}

/// The new offset on one axis: `None` for an omitted value, else the number of the value, `0`
/// for a non-finite one, added to `from` for a relative move.
fn axis<'js>(
    ctx: &Ctx<'js>,
    value: &Value<'js>,
    relative: bool,
    from: f64,
) -> JsResult<Option<f64>> {
    if value.is_undefined() {
        return Ok(None);
    }
    let amount = Coerced::<f64>::from_js(ctx, value.clone())?.0;
    let amount = if amount.is_finite() { amount } else { 0.0 };
    Ok(Some(if relative { from + amount } else { amount }))
}

/// Moves the scroll offset of the node `id`. The arguments are `(left, top)` or an options object.
fn move_scroll<'js>(
    ctx: &Ctx<'js>,
    host: &Shared,
    id: NodeId,
    args: &[Value<'js>],
    relative: bool,
) -> JsResult<()> {
    if !host.document.borrow().tree.is_connected(id) {
        return Ok(());
    }
    let undefined = Value::new_undefined(ctx.clone());
    let first = args.first().unwrap_or(&undefined);
    let is_options = first.is_null() || (first.is_object() && !first.is_function());
    let (x, y) = if is_options {
        (
            option_of(ctx, Some(first), "left")?,
            option_of(ctx, Some(first), "top")?,
        )
    } else {
        (first.clone(), args.get(1).unwrap_or(&undefined).clone())
    };
    let [left, top, ..] = scroll_of(host, id).unwrap_or([0.0; 4]);
    let target = (
        axis(ctx, &x, relative, left)?,
        axis(ctx, &y, relative, top)?,
    );
    scroll_to(host, id, target.0, target.1);
    Ok(())
}

/// The alignment `value` names, `default` for any other value.
fn alignment(value: &Value<'_>, default: Align) -> Align {
    match value
        .as_string()
        .and_then(|name| name.to_string().ok())
        .as_deref()
    {
        Some("start") => Align::Start,
        Some("center") => Align::Center,
        Some("end") => Align::End,
        Some("nearest") => Align::Nearest,
        _ => default,
    }
}

/// The `(block, inline)` alignments of the argument of `scrollIntoView`.
fn alignments<'js>(ctx: &Ctx<'js>, arg: Option<&Value<'js>>) -> JsResult<(Align, Align)> {
    match arg {
        Some(arg) if arg.is_object() => Ok((
            alignment(&option_of(ctx, Some(arg), "block")?, Align::Start),
            alignment(&option_of(ctx, Some(arg), "inline")?, Align::Nearest),
        )),
        Some(arg) if arg.as_bool() == Some(false) => Ok((Align::End, Align::Nearest)),
        _ => Ok((Align::Start, Align::Nearest)),
    }
}

/// Defines the scroll members on `proto`, the prototype of `Element`.
pub(super) fn install<'js>(ctx: &Ctx<'js>, proto: &Object<'js>, host: &Rc<Host>) -> JsResult<()> {
    accessor(
        ctx,
        proto,
        host,
        "scrollLeft",
        &ELEMENT,
        |_, host, id, _| Ok(scroll_of(host, id).map_or(0.0, |s| s[0])),
        |ctx, host, id, _, value| move_scroll(ctx, host, id, &[value], false),
    )?;
    accessor(
        ctx,
        proto,
        host,
        "scrollTop",
        &ELEMENT,
        |_, host, id, _| Ok(scroll_of(host, id).map_or(0.0, |s| s[1])),
        |ctx, host, id, _, value| {
            let undefined = Value::new_undefined(ctx.clone());
            move_scroll(ctx, host, id, &[undefined, value], false)
        },
    )?;
    getter(
        ctx,
        proto,
        host,
        "scrollWidth",
        &ELEMENT,
        |_, host, id, _| {
            Ok(client_width(host, id) + scroll_of(host, id).map_or(0.0, |s| rounded(s[2])))
        },
    )?;
    getter(
        ctx,
        proto,
        host,
        "scrollHeight",
        &ELEMENT,
        |_, host, id, _| {
            Ok(client_height(host, id) + scroll_of(host, id).map_or(0.0, |s| rounded(s[3])))
        },
    )?;
    method(
        ctx,
        proto,
        host,
        "scrollTo",
        0,
        &ELEMENT,
        |ctx, host, id, _, args| move_scroll(ctx, host, id, args, false),
    )?;
    method(
        ctx,
        proto,
        host,
        "scrollBy",
        0,
        &ELEMENT,
        |ctx, host, id, _, args| move_scroll(ctx, host, id, args, true),
    )?;
    method(
        ctx,
        proto,
        host,
        "scrollIntoView",
        0,
        &ELEMENT,
        |ctx, host, id, _, args| {
            if !host.document.borrow().tree.is_connected(id) {
                return Ok(());
            }
            let (block, inline) = alignments(ctx, args.first())?;
            scroll_into_view(host, id, block, inline);
            Ok(())
        },
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use inca_document::geometry::Align;
    use rstest::rstest;

    use super::super::fixture::with_box_view;
    use crate::test_support::TestView;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// The offset the last scroll write left on the node `id`.
    fn offset(view: &Rc<RefCell<TestView>>, id: u32) -> (f32, f32) {
        view.borrow()
            .offsets
            .borrow()
            .get(&id)
            .copied()
            .unwrap_or_default()
    }

    #[rstest]
    #[case::freed("__inca_native__.destroyNode(id)")]
    #[case::detached("__inca_native__.removeChild(__inca_native__.rootNodeId(), id)")]
    fn a_scroll_write_to_a_node_without_a_box_changes_nothing(#[case] remove: &str) {
        let (engine, _host, view, id) = with_box_view([0.0; 9]);
        engine.eval::<()>(remove).unwrap();

        engine
            .eval::<()>(
                r"
                el.scrollTop = 5;
                el.scrollLeft = 5;
                el.scrollTo(1, 1);
                el.scrollBy(1, 1);
                ",
            )
            .unwrap();

        assert_eq!(offset(&view, id), (0.0, 0.0));
    }

    #[rstest]
    #[case::freed("__inca_native__.destroyNode(id)")]
    #[case::detached("__inca_native__.removeChild(__inca_native__.rootNodeId(), id)")]
    fn a_scroll_into_view_of_a_node_without_a_box_waits_for_nothing(#[case] remove: &str) {
        let (engine, _host, view, id) = with_box_view([0.0; 9]);
        engine.eval::<()>(remove).unwrap();
        view.borrow_mut().boxes.remove(&id);

        engine.eval::<()>("el.scrollIntoView()").unwrap();

        assert!(view.borrow().queued.is_empty());
    }

    #[test]
    fn scroll_to_moves_the_offset_with_numbers_and_with_options() {
        let (engine, _host, view, id) = with_box_view([0.0; 9]);
        let offset = || offset(&view, id);

        engine.eval::<()>("el.scrollTo(5, 7)").unwrap();
        assert_eq!(offset(), (5.0, 7.0));
        engine.eval::<()>("el.scrollTo({ top: 9 })").unwrap();
        assert_eq!(offset(), (5.0, 9.0));
        engine
            .eval::<()>(
                r"
                el.scrollTop = 2;
                el.scrollLeft = 3;
                ",
            )
            .unwrap();
        assert_eq!(offset(), (3.0, 2.0));
        engine.eval::<()>("el.scrollTo(NaN, Infinity)").unwrap();
        assert_eq!(offset(), (0.0, 0.0));
    }

    #[test]
    fn scroll_by_adds_to_the_current_offset() {
        let (engine, _host, view, id) = with_box_view([0.0; 9]);
        engine.eval::<()>("el.scrollBy(2, 3)").unwrap();
        let first = offset(&view, id);
        engine.eval::<()>("el.scrollBy({ left: 1 })").unwrap();
        let second = offset(&view, id);

        // The maximum offset is zero here, so the base is the clamped offset 0.
        assert_eq!((first.0, first.1, second.0), (2.0, 3.0, 1.0));
    }

    #[rstest]
    #[case::default("el.scrollIntoView()", Align::Start, Align::Nearest)]
    #[case::true_value("el.scrollIntoView(true)", Align::Start, Align::Nearest)]
    #[case::false_value("el.scrollIntoView(false)", Align::End, Align::Nearest)]
    #[case::empty_options("el.scrollIntoView({})", Align::Start, Align::Nearest)]
    #[case::start_and_end(
        "el.scrollIntoView({ block: 'start', inline: 'end' })",
        Align::Start,
        Align::End
    )]
    #[case::center_and_unknown(
        "el.scrollIntoView({ block: 'center', inline: 'bogus' })",
        Align::Center,
        Align::Nearest
    )]
    #[case::nearest_and_center(
        "el.scrollIntoView({ block: 'nearest', inline: 'center' })",
        Align::Nearest,
        Align::Center
    )]
    fn scroll_into_view_waits_for_a_box_with_the_alignments_asked(
        #[case] call: &str,
        #[case] block: Align,
        #[case] inline: Align,
    ) {
        let (engine, _host, view, id) = with_box_view([0.0; 9]);
        view.borrow_mut().boxes.remove(&id);

        engine.eval::<()>(call).unwrap();

        assert_eq!(view.borrow().queued, [(id, block, inline)]);
    }

    #[test]
    fn scroll_into_view_of_a_node_with_a_box_leaves_nothing_waiting() {
        let (engine, _host, view, _id) = with_box_view([0.0; 9]);

        engine.eval::<()>("el.scrollIntoView()").unwrap();

        assert!(view.borrow().queued.is_empty());
    }
}
