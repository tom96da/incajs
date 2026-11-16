// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The classes `AbortController` and `AbortSignal`, their prototypes and members.

use std::cell::RefCell;
use std::rc::Rc;

use inca_jsenv::class::{
    define_class, define_global, define_this_getter, define_this_method, new_target_prototype,
    receiver,
};
use inca_jsenv::{required_prototype, store_prototype, stored_prototype};
use rquickjs::class::{JsClass, Readable, Trace, Tracer};
use rquickjs::function::{Constructor, IntoJsFunc, ParamRequirement, Params};
use rquickjs::{Class, Ctx, Exception, Function, Object, Result as JsResult, Value};

use super::table::Table;
use super::{abort_signal, new_signal, statics};
use crate::bindings::Host;
use crate::interfaces::define_unconstructable;

pub(crate) use instance::{IncaController, IncaSignal};

// A derive on a struct with a lifetime generates items that an `allow` on the struct does not
// cover, so the structs sit in a module of their own.
#[allow(clippy::disallowed_macros, clippy::disallowed_types)]
mod instance {
    use std::cell::RefCell;

    use rquickjs::{Class, JsLifetime, Value};

    /// An `AbortSignal` object. The reason is traced, so a reason that refers to its signal is a
    /// cycle the engine collects.
    #[derive(JsLifetime)]
    pub(crate) struct IncaSignal<'js> {
        pub(super) id: u32,
        pub(super) reason: RefCell<Option<Value<'js>>>,
    }

    /// An `AbortController` object. The signal is traced, so a script that holds the controller
    /// keeps the signal in the weak table.
    #[derive(JsLifetime)]
    pub(crate) struct IncaController<'js> {
        pub(super) signal: Class<'js, IncaSignal<'js>>,
    }
}

impl<'js> IncaSignal<'js> {
    pub(super) fn new(id: u32) -> Self {
        Self {
            id,
            reason: RefCell::new(None),
        }
    }

    /// The id of the signal in `Signals`.
    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    /// The stored reason, `None` before the abort.
    pub(super) fn reason(&self) -> Option<Value<'js>> {
        self.reason.borrow().clone()
    }

    pub(super) fn set_reason(&self, value: Value<'js>) {
        *self.reason.borrow_mut() = Some(value);
    }
}

impl<'js> Trace<'js> for IncaSignal<'js> {
    // Runs inside a GC callback of QuickJS: it must not panic.
    fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
        // No `..` here: a new field must fail to compile until it is traced.
        let IncaSignal { id: _, reason } = self;
        if let Ok(reason) = reason.try_borrow() {
            reason.trace(tracer);
        }
    }
}

impl<'js> Trace<'js> for IncaController<'js> {
    // Runs inside a GC callback of QuickJS: it must not panic.
    fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
        let IncaController { signal } = self;
        signal.trace(tracer);
    }
}

impl<'js> JsClass<'js> for IncaSignal<'js> {
    const NAME: &'static str = "AbortSignal";

    type Mutable = Readable;

    // An instance takes the prototype from the vault when it is made.
    fn prototype(_ctx: &Ctx<'js>) -> JsResult<Option<Object<'js>>> {
        Ok(None)
    }

    fn constructor(_ctx: &Ctx<'js>) -> JsResult<Option<Constructor<'js>>> {
        Ok(None)
    }
}

impl<'js> JsClass<'js> for IncaController<'js> {
    const NAME: &'static str = "AbortController";

    type Mutable = Readable;

    // An instance takes the prototype from the vault when it is made.
    fn prototype(_ctx: &Ctx<'js>) -> JsResult<Option<Object<'js>>> {
        Ok(None)
    }

    fn constructor(_ctx: &Ctx<'js>) -> JsResult<Option<Constructor<'js>>> {
        Ok(None)
    }
}

/// The body of the `AbortController` constructor.
struct Construct {
    host: Rc<RefCell<Host>>,
}

impl<'js> IntoJsFunc<'js, ()> for Construct {
    fn param_requirements() -> ParamRequirement {
        ParamRequirement::any()
    }

    fn call<'a>(&self, params: Params<'a, 'js>) -> JsResult<Value<'js>> {
        let ctx = params.ctx();
        if !params.is_constructor() {
            return Err(Exception::throw_type(
                ctx,
                "AbortController needs new. write new AbortController()",
            ));
        }
        let table = Table::open(ctx)?;
        let signal = new_signal(ctx, &self.host, &table, false)?;
        let proto = new_target_prototype(&params, required_prototype(ctx, "AbortController")?)?;
        let controller = IncaController { signal };
        Ok(Class::instance_proto(controller, proto)?.into_value())
    }
}

/// Defines both classes and the table.
pub(super) fn install<'js>(ctx: &Ctx<'js>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
    let Some(event_target) = stored_prototype(ctx, "EventTarget")? else {
        return Err(Exception::throw_type(
            ctx,
            "EventTarget is not defined. install the node interfaces first",
        ));
    };
    let on_collect = {
        let host = Rc::clone(host);
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, id: u32| {
            super::release_signal(&ctx, &host, id)
        })?
    };
    Table::create(ctx, on_collect)?;
    define_controller(ctx, host)?;
    define_signal(ctx, host, &event_target)
}

fn define_controller(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
    let proto = Object::new(ctx.clone())?;
    let class = define_class(
        ctx,
        &proto,
        "AbortController",
        0,
        Construct {
            host: Rc::clone(host),
        },
    )?;
    define_this_getter(ctx, &proto, "signal", |ctx, this| {
        Ok(receiver::<IncaController>(ctx, &this)?
            .borrow()
            .signal
            .clone())
    })?;
    let host = Rc::clone(host);
    define_this_method(ctx, &proto, "abort", 0, move |ctx, this, args| {
        let controller = receiver::<IncaController>(ctx, &this)?;
        let signal = controller.borrow().signal.clone();
        let reason = args.first().filter(|v| !v.is_undefined()).cloned();
        abort_signal(ctx, &host, &Table::open(ctx)?, &signal, reason)
    })?;
    define_global(ctx, "AbortController", class)?;
    store_prototype(ctx, "AbortController", &proto)
}

fn define_signal<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    event_target: &Object<'js>,
) -> JsResult<()> {
    let proto = Object::new(ctx.clone())?;
    let state = Rc::clone(host);
    define_this_getter(ctx, &proto, "aborted", move |ctx, this| {
        let signal = receiver::<IncaSignal>(ctx, &this)?;
        let id = signal.borrow().id;
        Ok(state.borrow().signals.is_aborted(id))
    })?;
    define_this_getter(ctx, &proto, "reason", |ctx, this| {
        let signal = receiver::<IncaSignal>(ctx, &this)?;
        let reason = signal.borrow().reason();
        Ok(reason.unwrap_or_else(|| Value::new_undefined(ctx.clone())))
    })?;
    define_this_method(ctx, &proto, "throwIfAborted", 0, |ctx, this, _| {
        let signal = receiver::<IncaSignal>(ctx, &this)?;
        let reason = signal.borrow().reason();
        match reason {
            Some(reason) => Err::<(), _>(ctx.throw(reason)),
            None => Ok(()),
        }
    })?;
    define_unconstructable(
        ctx,
        &proto,
        "AbortSignal",
        "an AbortSignal cannot be constructed. read one from an AbortController or call AbortSignal.abort()",
    )?;
    let class: Object = ctx.globals().get("AbortSignal")?;
    let parent: Object = ctx.globals().get("EventTarget")?;
    proto.set_prototype(Some(event_target))?;
    class.set_prototype(Some(&parent))?;
    statics::define(ctx, host, &class)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::{engine_with_bindings, error_name, json_of};

    fn json(expr: &str) -> serde_json::Value {
        let (engine, _host) = engine_with_bindings();
        json_of(&engine, expr)
    }

    /// The letters of the attributes of `owner[key]`, as in `w` for writable.
    fn attributes(owner: &str, key: &str) -> String {
        let (engine, _host) = engine_with_bindings();
        engine
            .eval(&format!(
                r"
                (() => {{
                    const d = Object.getOwnPropertyDescriptor({owner}, {key});
                    return [d.writable ? 'w' : '-', d.enumerable ? 'e' : '-',
                        d.configurable ? 'c' : '-', d.get ? 'g' : '-', d.set ? 's' : '-'].join('');
                }})()
                "
            ))
            .unwrap()
    }

    #[rstest]
    #[case::controller("AbortController")]
    #[case::signal("AbortSignal")]
    fn a_global_class_is_writable_configurable_and_not_enumerable(#[case] name: &str) {
        assert_eq!(attributes("globalThis", &format!("'{name}'")), "w-c--");
    }

    #[rstest]
    #[case::controller_signal("AbortController.prototype", "'signal'", "-ecg-")]
    #[case::controller_abort("AbortController.prototype", "'abort'", "wec--")]
    #[case::aborted("AbortSignal.prototype", "'aborted'", "-ecg-")]
    #[case::reason("AbortSignal.prototype", "'reason'", "-ecg-")]
    #[case::throw_if_aborted("AbortSignal.prototype", "'throwIfAborted'", "wec--")]
    #[case::abort("AbortSignal", "'abort'", "wec--")]
    #[case::timeout("AbortSignal", "'timeout'", "wec--")]
    #[case::any("AbortSignal", "'any'", "wec--")]
    #[case::tag("AbortSignal.prototype", "Symbol.toStringTag", "--c--")]
    fn a_member_has_the_attributes_of_its_kind(
        #[case] owner: &str,
        #[case] key: &str,
        #[case] want: &str,
    ) {
        assert_eq!(attributes(owner, key), want);
    }

    #[rstest]
    #[case::controller("AbortController", json!([0, "AbortController"]))]
    #[case::signal("AbortSignal", json!([0, "AbortSignal"]))]
    #[case::controller_abort("AbortController.prototype.abort", json!([0, "abort"]))]
    #[case::throw_if_aborted("AbortSignal.prototype.throwIfAborted", json!([0, "throwIfAborted"]))]
    #[case::abort("AbortSignal.abort", json!([0, "abort"]))]
    #[case::timeout("AbortSignal.timeout", json!([1, "timeout"]))]
    #[case::any("AbortSignal.any", json!([1, "any"]))]
    #[case::signal_getter(
        "Object.getOwnPropertyDescriptor(AbortController.prototype, 'signal').get",
        json!([0, "get signal"])
    )]
    fn a_function_has_its_length_and_name(#[case] path: &str, #[case] want: serde_json::Value) {
        assert_eq!(json(&format!("[{path}.length, {path}.name]")), want);
    }

    #[rstest]
    #[case::controller("new AbortController()", "[object AbortController]")]
    #[case::signal("new AbortController().signal", "[object AbortSignal]")]
    fn the_string_tags_name_the_classes(#[case] value: &str, #[case] want: &str) {
        let got = json(&format!("Object.prototype.toString.call({value})"));

        assert_eq!(got, json!(want));
    }

    #[test]
    fn a_signal_inherits_from_the_event_target() {
        let got = json(
            r"[Object.getPrototypeOf(AbortSignal.prototype) === EventTarget.prototype,
                Object.getPrototypeOf(AbortSignal) === EventTarget]",
        );

        assert_eq!(got, json!([true, true]));
    }

    #[test]
    fn the_own_names_of_the_prototypes_are_the_members() {
        let got = json(
            r"[AbortController.prototype, AbortSignal.prototype, AbortSignal]
                .map((o) => Object.getOwnPropertyNames(o).filter((n) => !['length', 'name', 'prototype', 'constructor'].includes(n)).sort())",
        );

        assert_eq!(
            got,
            json!([
                ["abort", "signal"],
                ["aborted", "reason", "throwIfAborted"],
                ["abort", "any", "timeout"]
            ])
        );
    }

    #[rstest]
    #[case::signal_new("new AbortSignal()")]
    #[case::signal_call("AbortSignal()")]
    #[case::controller_call("AbortController()")]
    fn constructing_wrongly_throws_a_type_error_with_the_message(#[case] call: &str) {
        let got = json(&format!(
            r"(() => {{
                try {{
                    {call};
                }} catch (e) {{
                    return [e.name, e.message];
                }}
            }})()"
        ));

        let message = if call.contains("Controller") {
            "AbortController needs new. write new AbortController()"
        } else {
            "an AbortSignal cannot be constructed. read one from an AbortController or call AbortSignal.abort()"
        };
        assert_eq!(got, json!(["TypeError", message]));
    }

    #[test]
    fn the_signal_is_the_same_object_each_time() {
        let got = json(
            r"(() => {
                    const c = new AbortController();
                    return c.signal === c.signal;
                })()",
        );

        assert_eq!(got, json!(true));
    }

    #[test]
    fn a_subclass_keeps_its_prototype_and_gets_a_signal() {
        let got = json(
            r"(() => {
                class K extends AbortController {}
                const k = new K();
                return [k instanceof K, Object.getPrototypeOf(k) === K.prototype,
                    Object.prototype.toString.call(k.signal)];
            })()",
        );

        assert_eq!(got, json!([true, true, "[object AbortSignal]"]));
    }

    #[rstest]
    #[case::signal_getter_on_forged_controller(
        "Object.getOwnPropertyDescriptor(AbortController.prototype, 'signal').get",
        "Object.create(AbortController.prototype)"
    )]
    #[case::abort_on_forged_controller(
        "AbortController.prototype.abort",
        "Object.create(AbortController.prototype)"
    )]
    #[case::aborted_on_forged_signal(
        "Object.getOwnPropertyDescriptor(AbortSignal.prototype, 'aborted').get",
        "Object.create(AbortSignal.prototype)"
    )]
    #[case::reason_on_forged_signal(
        "Object.getOwnPropertyDescriptor(AbortSignal.prototype, 'reason').get",
        "Object.create(AbortSignal.prototype)"
    )]
    #[case::throw_if_aborted_on_forged_signal(
        "AbortSignal.prototype.throwIfAborted",
        "Object.create(AbortSignal.prototype)"
    )]
    #[case::signal_getter_on_a_signal(
        "Object.getOwnPropertyDescriptor(AbortController.prototype, 'signal').get",
        "new AbortController().signal"
    )]
    #[case::aborted_on_a_controller(
        "Object.getOwnPropertyDescriptor(AbortSignal.prototype, 'aborted').get",
        "new AbortController()"
    )]
    fn a_member_throws_a_type_error_for_a_forged_or_wrong_class_receiver(
        #[case] member: &str,
        #[case] receiver: &str,
    ) {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(&engine, &format!("({member}).call({receiver})")),
            "TypeError"
        );
    }

    #[test]
    fn the_receiver_message_names_the_class_with_an_article() {
        let got = json(
            r"(() => {
                const message = (f, r) => {
                    try {
                        f.call(r);
                    } catch (e) {
                        return e.message;
                    }
                };
                return [
                    message(AbortSignal.prototype.throwIfAborted, {}),
                    message(AbortController.prototype.abort, {}),
                ];
            })()",
        );

        assert_eq!(
            got,
            json!([
                "the receiver is not an AbortSignal. call the method on an instance",
                "the receiver is not an AbortController. call the method on an instance"
            ])
        );
    }

    #[test]
    fn the_statics_run_with_a_detached_receiver() {
        let got = json(
            r"(() => {
                const { abort, timeout, any } = AbortSignal;
                return [abort().aborted, timeout(5).aborted, any([]).aborted];
            })()",
        );

        assert_eq!(got, json!([true, false, false]));
    }

    #[rstest]
    #[case::constructor("new AbortController().signal")]
    #[case::abort("AbortSignal.abort()")]
    #[case::timeout("AbortSignal.timeout(5)")]
    #[case::any("AbortSignal.any([])")]
    fn the_native_signal_id_gives_a_number_for_a_signal(#[case] source: &str) {
        let got = json(&format!("typeof __inca_native__.signalId({source})"));

        assert_eq!(got, json!("number"));
    }
}
