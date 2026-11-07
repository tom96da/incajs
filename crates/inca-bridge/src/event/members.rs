// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use inca_jsenv::FunctionAccessor;
use inca_jsenv::class::{
    define_method, define_this_accessor, define_this_getter, define_this_method, getter_function,
};
use rquickjs::convert::Coerced;
use rquickjs::function::{Opt, This};
use rquickjs::{Ctx, Function, IntoJs, Object, Result as JsResult, Value};

use super::{array_of, constants, given, initialize, with_event};
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
    getter_function(ctx, "isTrusted", |ctx, this| {
        with_event(ctx, &this, |event| event.state.trusted.get().into_js(ctx))
    })
}

fn define_methods<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    define_this_method(ctx, proto, "stopPropagation", |ctx, this| {
        with_event(ctx, &this, |event| {
            event.state.stop.set(true);
            Ok(())
        })
    })?;
    define_this_method(ctx, proto, "stopImmediatePropagation", |ctx, this| {
        with_event(ctx, &this, |event| {
            event.state.stop.set(true);
            event.state.immediate.set(true);
            Ok(())
        })
    })?;
    define_this_method(ctx, proto, "preventDefault", |ctx, this| {
        with_event(ctx, &this, |event| {
            if event.cancels() {
                event.state.canceled.set(true);
            }
            Ok(())
        })
    })?;
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
    define_this_method(ctx, proto, "composedPath", |ctx, this| {
        with_event(ctx, &this, |event| {
            let path = if event.state.dispatching.get() {
                event.held.path.borrow().clone()
            } else {
                Vec::new()
            };
            array_of(ctx, path)
        })
    })
}

fn define_fields<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    define_this_getter(ctx, proto, "type", |ctx, this| {
        with_event(ctx, &this, |event| {
            event.state.name.borrow().as_str().into_js(ctx)
        })
    })?;
    define_this_getter(ctx, proto, "target", |ctx, this| {
        with_event(ctx, &this, |event| {
            let target = event.held.target.borrow().clone();
            or_null(ctx, target)
        })
    })?;
    define_this_getter(ctx, proto, "currentTarget", |ctx, this| {
        with_event(ctx, &this, |event| or_null(ctx, event.current_target()))
    })?;
    define_this_getter(ctx, proto, "eventPhase", |ctx, this| {
        with_event(ctx, &this, |event| event.state.phase.get().into_js(ctx))
    })?;
    define_this_getter(ctx, proto, "bubbles", |ctx, this| {
        with_event(ctx, &this, |event| {
            event.state.flags.get().bubbles.into_js(ctx)
        })
    })?;
    define_this_getter(ctx, proto, "cancelable", |ctx, this| {
        with_event(ctx, &this, |event| {
            event.state.flags.get().cancelable.into_js(ctx)
        })
    })?;
    define_this_getter(ctx, proto, "defaultPrevented", |ctx, this| {
        with_event(ctx, &this, |event| event.state.canceled.get().into_js(ctx))
    })?;
    define_this_getter(ctx, proto, "composed", |ctx, this| {
        with_event(ctx, &this, |event| {
            event.state.flags.get().composed.into_js(ctx)
        })
    })?;
    define_this_getter(ctx, proto, "timeStamp", |ctx, this| {
        with_event(ctx, &this, |event| event.state.time_stamp.into_js(ctx))
    })?;
    define_this_getter(ctx, proto, "srcElement", |ctx, this| {
        with_event(ctx, &this, |event| {
            let target = event.held.target.borrow().clone();
            or_null(ctx, target)
        })
    })?;
    define_this_accessor(
        ctx,
        proto,
        "returnValue",
        |ctx, this| {
            with_event(ctx, &this, |event| {
                (!event.state.canceled.get()).into_js(ctx)
            })
        },
        |ctx, this, value: Coerced<bool>| {
            with_event(ctx, &this, |event| {
                if !value.0 && event.cancels() {
                    event.state.canceled.set(true);
                }
                Ok(())
            })
        },
    )?;
    define_this_accessor(
        ctx,
        proto,
        "cancelBubble",
        |ctx, this| with_event(ctx, &this, |event| event.state.stop.get().into_js(ctx)),
        |ctx, this, value: Coerced<bool>| {
            with_event(ctx, &this, |event| {
                if value.0 {
                    event.state.stop.set(true);
                }
                Ok(())
            })
        },
    )?;
    // The host's own field, so it stays out of `for...in`.
    let event_id = getter_function(ctx, "eventId", |ctx, this| {
        with_event(ctx, &this, |event| event.state.event_id.get().into_js(ctx))
    })?;
    proto.prop(
        "eventId",
        FunctionAccessor::new_get(event_id).configurable(),
    )
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
