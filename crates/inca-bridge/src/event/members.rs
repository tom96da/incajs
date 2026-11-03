// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use inca_jsenv::FunctionAccessor;
use inca_jsenv::class::define_method;
use rquickjs::convert::Coerced;
use rquickjs::function::{Opt, This};
use rquickjs::{Ctx, Function, IntoJs, Object, Result as JsResult, Value};

use super::{IncaEvent, action, array_of, constants, field, getter, given, initialize, with_event};
use crate::bindings::convert::or_null;

/// The phase constants on `Event` and its prototype.
pub(super) const PHASES: [(&str, i32); 4] = [
    ("NONE", 0),
    ("CAPTURING_PHASE", 1),
    ("AT_TARGET", 2),
    ("BUBBLING_PHASE", 3),
];

/// `Event.prototype`: the phase constants, the propagation methods,
/// `composedPath`, `initEvent` and the getters of the base fields.
pub(super) fn event_prototype<'js>(ctx: &Ctx<'js>) -> JsResult<Object<'js>> {
    let proto = Object::new(ctx.clone())?;
    constants(&proto, &PHASES)?;
    define_methods(ctx, &proto)?;
    define_fields(ctx, &proto)?;
    Ok(proto)
}

/// The getter every event owns as `isTrusted`.
pub(super) fn is_trusted<'js>(ctx: &Ctx<'js>) -> JsResult<Function<'js>> {
    getter(ctx, "isTrusted", |ctx, event| {
        event.state.trusted.get().into_js(ctx)
    })
}

fn define_methods<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    action(ctx, proto, "stopPropagation", |event| {
        event.state.stop.set(true);
    })?;
    action(ctx, proto, "stopImmediatePropagation", |event| {
        event.state.stop.set(true);
        event.state.immediate.set(true);
    })?;
    action(ctx, proto, "preventDefault", |event| {
        if event.cancels() {
            event.state.canceled.set(true);
        }
    })?;
    let composed_path = Function::new(
        ctx.clone(),
        |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
            with_event(&ctx, &this, |event| {
                let path = if event.state.dispatching.get() {
                    event.held.path.borrow().clone()
                } else {
                    Vec::new()
                };
                array_of(&ctx, path)
            })
        },
    )?;
    let init_event = Function::new(
        ctx.clone(),
        |ctx: Ctx<'js>,
         This(this): This<Value<'js>>,
         event_type: Coerced<String>,
         bubbles: Opt<Coerced<bool>>,
         cancelable: Opt<Coerced<bool>>| {
            with_event(&ctx, &this, |event| {
                initialize(event, event_type.0, given(&bubbles), given(&cancelable));
                Ok(())
            })
        },
    )?;
    define_method(proto, "initEvent", init_event)?;
    define_method(proto, "composedPath", composed_path)
}

fn define_fields<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    field(ctx, proto, "type", |ctx, event| {
        event.state.name.borrow().as_str().into_js(ctx)
    })?;
    field(ctx, proto, "target", |ctx, event| {
        let target = event.held.target.borrow().clone();
        or_null(ctx, target)
    })?;
    field(ctx, proto, "currentTarget", |ctx, event| {
        or_null(ctx, event.current_target())
    })?;
    field(ctx, proto, "eventPhase", |ctx, event| {
        event.state.phase.get().into_js(ctx)
    })?;
    field(ctx, proto, "bubbles", |ctx, event| {
        event.state.flags.get().bubbles.into_js(ctx)
    })?;
    field(ctx, proto, "cancelable", |ctx, event| {
        event.state.flags.get().cancelable.into_js(ctx)
    })?;
    field(ctx, proto, "defaultPrevented", |ctx, event| {
        event.state.canceled.get().into_js(ctx)
    })?;
    field(ctx, proto, "composed", |ctx, event| {
        event.state.flags.get().composed.into_js(ctx)
    })?;
    field(ctx, proto, "timeStamp", |ctx, event| {
        event.state.time_stamp.into_js(ctx)
    })?;
    field(ctx, proto, "srcElement", |ctx, event| {
        let target = event.held.target.borrow().clone();
        or_null(ctx, target)
    })?;
    let return_value = getter(ctx, "returnValue", |ctx, event| {
        (!event.state.canceled.get()).into_js(ctx)
    })?;
    let set_return_value = setter(ctx, "returnValue", |event, value| {
        if !value && event.cancels() {
            event.state.canceled.set(true);
        }
    })?;
    proto.prop(
        "returnValue",
        FunctionAccessor::new_get(return_value)
            .set(set_return_value)
            .enumerable()
            .configurable(),
    )?;
    let cancel_bubble = getter(ctx, "cancelBubble", |ctx, event| {
        event.state.stop.get().into_js(ctx)
    })?;
    let set_cancel_bubble = setter(ctx, "cancelBubble", |event, value| {
        if value {
            event.state.stop.set(true);
        }
    })?;
    proto.prop(
        "cancelBubble",
        FunctionAccessor::new_get(cancel_bubble)
            .set(set_cancel_bubble)
            .enumerable()
            .configurable(),
    )?;
    // The host's own field, so it stays out of `for...in`.
    let event_id = getter(ctx, "eventId", |ctx, event| {
        event.state.event_id.get().into_js(ctx)
    })?;
    proto.prop(
        "eventId",
        FunctionAccessor::new_get(event_id).configurable(),
    )
}

/// The setter `set <key>`, which runs `write` with the value converted to a
/// boolean on the event `this` names.
fn setter<'js>(
    ctx: &Ctx<'js>,
    key: &str,
    write: fn(&IncaEvent<'_>, bool),
) -> JsResult<Function<'js>> {
    Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>, value: Coerced<bool>| {
            with_event(&ctx, &this, |event| {
                write(event, value.0);
                Ok(())
            })
        },
    )?
    .with_name(format!("set {key}"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::{
        at, dispatcher_with_engine, eval_json, fire_and_read, key_payload, listen, mouse_at,
        wheel_at,
    };
    use gpui::TestAppContext;
    use inca_gpui::{EventPayload, MousePayload};
    use inca_test_macros::gpui_cases;

    #[gpui::test]
    fn an_event_method_throws_a_type_error_for_any_receiver_that_is_not_an_event(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "click",
            r"
            (e) => {
            const proto = Object.getPrototypeOf(e);
            const getter = (name) => Object.getOwnPropertyDescriptor(proto, name).get;
            const setter = (name) => Object.getOwnPropertyDescriptor(proto, name).set;
            const calls = [
                e.stopPropagation, e.stopImmediatePropagation, e.preventDefault,
                e.composedPath, getter('defaultPrevented'), getter('returnValue'),
                getter('cancelBubble'), (r) => setter('returnValue').call(r, false),
                (r) => setter('cancelBubble').call(r, true),
            ];
            for (const receiver of [undefined, null, 5, {}, Object.create(e), proto]) {
                for (const call of calls) {
                    try { call.call(receiver, receiver); out.push('ran'); }
                    catch (err) { out.push(err instanceof TypeError); }
                }
            }
            e.preventDefault();
            out.push(e.defaultPrevented); }
            ",
        );

        let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

        let mut want = vec![serde_json::json!(true); 6 * 9];
        want.push(serde_json::json!(true));
        assert_eq!(out, serde_json::Value::Array(want));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::mouse("click", mouse_at(1.0, 1.0), "[object MouseEvent]")]
    #[case::wheel("wheel", wheel_at(1.0, 1.0), "[object WheelEvent]")]
    #[case::key(
        "keydown",
        key_payload(gpui::Modifiers::default()),
        "[object KeyboardEvent]"
    )]
    #[case::focus("focus", EventPayload::Focus { related_target: None }, "[object FocusEvent]")]
    fn a_host_event_reads_its_class_as_its_string_tag(
        cx: &mut TestAppContext,
        event: &str,
        payload: EventPayload,
        want: &str,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            event,
            "(e) => out.push(Object.prototype.toString.call(e))",
        );

        let out = fire_and_read(cx, &dispatcher, node, event, &payload);

        assert_eq!(out, serde_json::json!([want]));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::shared_method("Event.prototype", "preventDefault")]
    #[case::modifier_lookup("Object.getPrototypeOf(e)", "getModifierState")]
    fn an_event_method_descriptor_reads_writable_enumerable_configurable(
        cx: &mut TestAppContext,
        owner: &str,
        key: &str,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "keydown",
            &r"
            (e) => {
                const d = Object.getOwnPropertyDescriptor(OWNER, 'KEY');
                out.push([d.writable, d.enumerable, d.configurable].join());
            }
            "
            .replace("OWNER", owner)
            .replace("KEY", key),
        );

        let out = fire_and_read(
            cx,
            &dispatcher,
            node,
            "keydown",
            &key_payload(gpui::Modifiers::default()),
        );

        assert_eq!(out, serde_json::json!(["true,true,true"]));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::method("Event.prototype.preventDefault", "preventDefault")]
    #[case::modifier_lookup("Object.getPrototypeOf(e).getModifierState", "getModifierState")]
    #[case::getter(
        "Object.getOwnPropertyDescriptor(Event.prototype, 'srcElement').get",
        "get srcElement"
    )]
    #[case::setter(
        "Object.getOwnPropertyDescriptor(Event.prototype, 'returnValue').set",
        "set returnValue"
    )]
    fn an_event_function_carries_the_name_of_its_member(
        cx: &mut TestAppContext,
        function: &str,
        want: &str,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "keydown",
            &"(e) => out.push(FUNCTION.name)".replace("FUNCTION", function),
        );

        let out = fire_and_read(
            cx,
            &dispatcher,
            node,
            "keydown",
            &key_payload(gpui::Modifiers::default()),
        );

        assert_eq!(out, serde_json::json!([want]));
    }

    #[gpui::test]
    fn a_get_modifier_state_throws_type_error_for_an_event_of_another_kind(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        for event in ["click", "keydown"] {
            listen(
                &dispatcher,
                node,
                event,
                "(e) => (globalThis.kept ??= []).push(e)",
            );
        }
        let shift = gpui::Modifiers::shift();
        let mouse = EventPayload::Mouse(MousePayload::at(at(1.0, 1.0), shift));
        fire_and_read(cx, &dispatcher, node, "click", &mouse);
        fire_and_read(cx, &dispatcher, node, "keydown", &key_payload(shift));

        let got = eval_json(
            &dispatcher,
            r"
            (() => {
                const [mouse, key] = kept;
                const call = (f, receiver) => {
                    try {
                        return 'ran ' + f.call(receiver, 'Shift');
                    } catch (err) {
                        return err instanceof TypeError;
                    }
                };
                return [
                    call(mouse.getModifierState, key),
                    call(key.getModifierState, mouse),
                    call(mouse.getModifierState, new Event('x')),
                    call(mouse.getModifierState, mouse),
                    call(key.getModifierState, key)
                ];
            })()
            ",
        );

        assert_eq!(
            got,
            serde_json::json!([true, true, true, "ran true", "ran true"])
        );
    }
}
