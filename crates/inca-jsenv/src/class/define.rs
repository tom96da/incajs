// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The definition steps for the names a Web standard defines: a class with its constructor,
//! methods and getters, a free function and a namespace object.
//!
//! Each entry gives the attributes of its kind. Methods and accessors are enumerable.
//! [`define_function`] makes a writable, enumerable, configurable global. [`define_global`] (a
//! class) and [`define_namespace`] make a writable, configurable, non-enumerable one. A global
//! that is an accessor goes through [`define_accessor`] on the global object.
//!
//! The `this` entries ([`define_this_getter`], [`define_this_accessor`], [`define_this_method`],
//! [`getter_function`]) take a closure that gets the `this` value. The closure becomes the
//! function. The property gets the attributes and the function names of its kind.

use rquickjs::atom::PredefinedAtom;
use rquickjs::function::{IntoJsFunc, Rest, This};
use rquickjs::object::Property;
use rquickjs::{Ctx, FromJs, Function, IntoJs, Object, Result as JsResult, Value};

use crate::FunctionAccessor;

/// Emits the empty `Trace` and the `JsClass` of a class whose instances hold plain Rust data and
/// take their prototype from the vault.
#[macro_export]
macro_rules! plain_class {
    ($type:ident, $name:literal) => {
        impl<'js> ::rquickjs::class::Trace<'js> for $type {
            fn trace<'a>(&self, _tracer: ::rquickjs::class::Tracer<'a, 'js>) {}
        }

        impl<'js> ::rquickjs::class::JsClass<'js> for $type {
            const NAME: &'static str = $name;

            type Mutable = ::rquickjs::class::Readable;

            // An instance takes the prototype from the vault when it is made.
            fn prototype(
                _ctx: &::rquickjs::Ctx<'js>,
            ) -> ::rquickjs::Result<Option<::rquickjs::Object<'js>>> {
                Ok(None)
            }

            fn constructor(
                _ctx: &::rquickjs::Ctx<'js>,
            ) -> ::rquickjs::Result<Option<::rquickjs::function::Constructor<'js>>> {
                Ok(None)
            }
        }
    };
}

/// The prototype of an iterator class, derived from the iterator prototype that the results of
/// `Array.prototype[Symbol.iterator]` inherit from.
///
/// # Errors
///
/// Returns an error if the realm lacks the intrinsic.
pub fn iterator_prototype<'js>(ctx: &Ctx<'js>) -> JsResult<Object<'js>> {
    ctx.eval(ITERATOR_JS)
}

const ITERATOR_JS: &str = r"
    (() => {
        const iterators = Object.getPrototypeOf(Object.getPrototypeOf([][Symbol.iterator]()));
        return Object.create(iterators);
    })()
    ";

/// The constructor `name` with `length` parameters and the body `body`. `name.prototype` is
/// read-only, `proto.constructor` is writable and configurable, and `proto` gets the configurable
/// string tag `name`.
///
/// # Errors
///
/// Returns an error if a property cannot be defined.
pub fn define_class<'js, P>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &str,
    length: usize,
    body: impl IntoJsFunc<'js, P> + 'js,
) -> JsResult<Function<'js>> {
    let function = Function::new(ctx.clone(), body)?
        .with_name(name)?
        .with_constructor(true);
    function.set_length(length)?;
    function.prop("prototype", Property::from(proto.clone()))?;
    proto.prop(
        "constructor",
        Property::from(function.clone()).writable().configurable(),
    )?;
    proto.prop(
        PredefinedAtom::SymbolToStringTag,
        Property::from(name).configurable(),
    )?;
    Ok(function)
}

/// Defines `function` as the writable, enumerable, configurable method `name` of `target`. The
/// owner may be a prototype, a constructor or a namespace object.
///
/// # Errors
///
/// Returns an error if the property cannot be defined.
pub fn define_method<'js>(
    target: &Object<'js>,
    name: &str,
    function: Function<'js>,
) -> JsResult<()> {
    target.prop(
        name,
        Property::from(function.with_name(name)?)
            .writable()
            .enumerable()
            .configurable(),
    )
}

/// Defines `get` as the enumerable, configurable getter `key` of `target`, without a setter. The
/// function is named `get <key>`.
///
/// # Errors
///
/// Returns an error if the property cannot be defined.
pub fn define_getter<'js>(target: &Object<'js>, key: &str, get: Function<'js>) -> JsResult<()> {
    target.prop(
        key,
        FunctionAccessor::new_get(get.with_name(format!("get {key}"))?)
            .enumerable()
            .configurable(),
    )
}

/// Defines `get` and `set` as the enumerable, configurable accessor `key` of `target`. The
/// functions are named `get <key>` and `set <key>`.
///
/// # Errors
///
/// Returns an error if the property cannot be defined.
pub fn define_accessor<'js>(
    target: &Object<'js>,
    key: &str,
    get: Function<'js>,
    set: Function<'js>,
) -> JsResult<()> {
    target.prop(
        key,
        FunctionAccessor::new_get(get.with_name(format!("get {key}"))?)
            .set(set.with_name(format!("set {key}"))?)
            .enumerable()
            .configurable(),
    )
}

/// The function named `get <key>` that returns what `read` makes of the context and the `this`
/// value of the call.
///
/// # Errors
///
/// Returns an error if the function cannot be built.
pub fn getter_function<'js, R: IntoJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    key: &str,
    read: impl Fn(&Ctx<'js>, Value<'js>) -> JsResult<R> + 'js,
) -> JsResult<Function<'js>> {
    Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>| read(&ctx, this),
    )?
    .with_name(format!("get {key}"))
}

/// Defines the enumerable, configurable getter `key` of `target`, named `get <key>`. `read` gets
/// the context and the `this` value of the call.
///
/// # Errors
///
/// Returns an error if the function cannot be built or the property cannot be defined.
pub fn define_this_getter<'js, R: IntoJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    target: &Object<'js>,
    key: &str,
    read: impl Fn(&Ctx<'js>, Value<'js>) -> JsResult<R> + 'js,
) -> JsResult<()> {
    define_getter(target, key, getter_function(ctx, key, read)?)
}

/// Defines the enumerable, configurable accessor `key` of `target`, with the functions `get <key>`
/// and `set <key>`. `read` gets the context and the `this` value of the call. `write` gets the
/// context, the `this` value and the argument converted to `A`.
///
/// # Errors
///
/// Returns an error if a function cannot be built or the property cannot be defined.
pub fn define_this_accessor<'js, R: IntoJs<'js> + 'js, A: FromJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    target: &Object<'js>,
    key: &str,
    read: impl Fn(&Ctx<'js>, Value<'js>) -> JsResult<R> + 'js,
    write: impl Fn(&Ctx<'js>, Value<'js>, A) -> JsResult<()> + 'js,
) -> JsResult<()> {
    let set = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>, value: A| write(&ctx, this, value),
    )?;
    define_accessor(target, key, getter_function(ctx, key, read)?, set)
}

/// Defines the writable, enumerable, configurable method `name` of `target` with `length`
/// parameters. `run` gets the context, the `this` value and the arguments of the call. Returns the
/// method.
///
/// # Errors
///
/// Returns an error if the function cannot be built or the property cannot be defined.
pub fn define_this_method<'js, R: IntoJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    target: &Object<'js>,
    name: &str,
    length: usize,
    run: impl Fn(&Ctx<'js>, Value<'js>, &[Value<'js>]) -> JsResult<R> + 'js,
) -> JsResult<Function<'js>> {
    define_sized_method(
        ctx,
        target,
        name,
        length,
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            run(&ctx, this, &args)
        },
    )
}

/// Defines the method `name` of `target` like [`define_method`], from any function body, with
/// `length` parameters. Returns the method.
///
/// # Errors
///
/// Returns an error if the function cannot be built or the property cannot be defined.
pub fn define_sized_method<'js, P>(
    ctx: &Ctx<'js>,
    target: &Object<'js>,
    name: &str,
    length: usize,
    body: impl IntoJsFunc<'js, P> + 'js,
) -> JsResult<Function<'js>> {
    let function = Function::new(ctx.clone(), body)?.with_length(length)?;
    define_method(target, name, function.clone())?;
    Ok(function)
}

/// Defines `value` as the writable, configurable, non-enumerable global `name`.
///
/// # Errors
///
/// Returns an error if the global cannot be defined.
pub fn define_global<'js>(ctx: &Ctx<'js>, name: &str, value: Function<'js>) -> JsResult<()> {
    ctx.globals()
        .prop(name, Property::from(value).writable().configurable())
}

/// Defines `function`, named `name`, as the writable, enumerable, configurable global `name`.
///
/// # Errors
///
/// Returns an error if the global cannot be defined.
pub fn define_function<'js>(ctx: &Ctx<'js>, name: &str, function: Function<'js>) -> JsResult<()> {
    ctx.globals().prop(
        name,
        Property::from(function.with_name(name)?)
            .writable()
            .enumerable()
            .configurable(),
    )
}

/// Defines `object` as the writable, configurable, non-enumerable global `name`. Define the
/// members of `object` first.
///
/// # Errors
///
/// Returns an error if the global cannot be defined.
pub fn define_namespace<'js>(ctx: &Ctx<'js>, name: &str, object: Object<'js>) -> JsResult<()> {
    ctx.globals()
        .prop(name, Property::from(object).writable().configurable())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rquickjs::{Ctx, Function, Object, Value};
    use rstest::rstest;

    use super::*;
    use crate::tests::with_realm;

    /// Defines `Widget` on a fresh prototype `proto` with the method `run`, the getter `size` and
    /// the global `Widget`.
    fn define(ctx: &Ctx<'_>) {
        let proto = Object::new(ctx.clone()).unwrap();
        let widget = define_class(ctx, &proto, "Widget", 2, || ()).unwrap();
        define_method(&proto, "run", Function::new(ctx.clone(), || ()).unwrap()).unwrap();
        define_getter(&proto, "size", Function::new(ctx.clone(), || 3).unwrap()).unwrap();
        define_accessor(
            &proto,
            "level",
            Function::new(ctx.clone(), || 1).unwrap(),
            Function::new(ctx.clone(), || ()).unwrap(),
        )
        .unwrap();
        define_this_getter(ctx, &proto, "kind", |_, _| Ok(4)).unwrap();
        define_this_accessor(ctx, &proto, "mode", |_, _| Ok(5), |_, _, _: Value| Ok(())).unwrap();
        define_this_method(ctx, &proto, "go", 0, |_, _, _| Ok(())).unwrap();
        define_global(ctx, "Widget", widget).unwrap();
        define_function(ctx, "work", Function::new(ctx.clone(), || ()).unwrap()).unwrap();
        define_namespace(ctx, "space", Object::new(ctx.clone()).unwrap()).unwrap();
        ctx.globals().set("proto", proto).unwrap();
    }

    /// The attributes of the property `path` (an `[owner, key]` expression) as the letters `w`,
    /// `e`, `c`, `g` and `s` for writable, enumerable, configurable, getter and setter, and `-` for
    /// each one missing.
    fn attributes(ctx: &Ctx<'_>, path: &str) -> String {
        define(ctx);
        ctx.eval::<String, _>(format!(
            r"
            const [owner, key] = {path};
            const d = Object.getOwnPropertyDescriptor(owner, key);
            [
                d.writable ? 'w' : '-',
                d.enumerable ? 'e' : '-',
                d.configurable ? 'c' : '-',
                d.get ? 'g' : '-',
                d.set ? 's' : '-',
            ].join('');
            "
        ))
        .unwrap()
    }

    /// The string `source` evaluates to, after `define`.
    fn text(ctx: &Ctx<'_>, source: &str) -> String {
        define(ctx);
        ctx.eval::<String, _>(source).unwrap()
    }

    #[test]
    fn define_class_names_the_function() {
        with_realm(|ctx| {
            assert_eq!(text(&ctx, "Widget.name"), "Widget");
        });
    }

    #[test]
    fn define_class_sets_the_function_length() {
        with_realm(|ctx| {
            assert_eq!(text(&ctx, "String(Widget.length)"), "2");
        });
    }

    #[test]
    fn define_class_makes_the_prototype_property_read_only() {
        with_realm(|ctx| {
            assert_eq!(attributes(&ctx, "[Widget, 'prototype']"), "-----");
        });
    }

    #[test]
    fn define_class_makes_the_constructor_property_writable() {
        with_realm(|ctx| {
            assert_eq!(attributes(&ctx, "[proto, 'constructor']"), "w-c--");
        });
    }

    #[test]
    fn define_class_makes_the_string_tag_configurable() {
        with_realm(|ctx| {
            assert_eq!(attributes(&ctx, "[proto, Symbol.toStringTag]"), "--c--");
        });
    }

    #[test]
    fn define_method_makes_the_property_enumerable() {
        with_realm(|ctx| {
            assert_eq!(attributes(&ctx, "[proto, 'run']"), "wec--");
        });
    }

    #[test]
    fn define_method_names_the_function_after_the_method() {
        with_realm(|ctx| {
            assert_eq!(text(&ctx, "proto.run.name"), "run");
        });
    }

    #[test]
    fn define_getter_makes_an_enumerable_accessor_without_setter() {
        with_realm(|ctx| {
            assert_eq!(attributes(&ctx, "[proto, 'size']"), "-ecg-");
        });
    }

    #[test]
    fn define_getter_names_the_function_get_key() {
        with_realm(|ctx| {
            assert_eq!(
                text(
                    &ctx,
                    "Object.getOwnPropertyDescriptor(proto, 'size').get.name"
                ),
                "get size"
            );
        });
    }

    #[test]
    fn define_accessor_makes_an_enumerable_accessor_with_a_setter() {
        with_realm(|ctx| {
            assert_eq!(attributes(&ctx, "[proto, 'level']"), "-ecgs");
        });
    }

    #[rstest]
    #[case::getter("get", "get level")]
    #[case::setter("set", "set level")]
    fn define_accessor_names_the_functions_get_key_and_set_key(
        #[case] side: &str,
        #[case] expected: &str,
    ) {
        with_realm(|ctx| {
            assert_eq!(
                text(
                    &ctx,
                    &format!("Object.getOwnPropertyDescriptor(proto, 'level').{side}.name")
                ),
                expected
            );
        });
    }

    #[test]
    fn define_global_makes_the_property_not_enumerable() {
        with_realm(|ctx| {
            assert_eq!(attributes(&ctx, "[globalThis, 'Widget']"), "w-c--");
        });
    }

    #[test]
    fn define_function_makes_a_writable_enumerable_configurable_global() {
        with_realm(|ctx| {
            assert_eq!(attributes(&ctx, "[globalThis, 'work']"), "wec--");
        });
    }

    #[test]
    fn define_function_names_the_function_after_the_global() {
        with_realm(|ctx| {
            assert_eq!(text(&ctx, "work.name"), "work");
        });
    }

    #[test]
    fn define_namespace_makes_a_writable_configurable_non_enumerable_global() {
        with_realm(|ctx| {
            assert_eq!(attributes(&ctx, "[globalThis, 'space']"), "w-c--");
        });
    }

    #[test]
    fn define_this_getter_passes_the_this_value() {
        with_realm(|ctx| {
            define(&ctx);
            define_this_getter(
                &ctx,
                &ctx.globals().get("proto").unwrap(),
                "me",
                |_, this| Ok(this),
            )
            .unwrap();

            let got = ctx
                .eval::<bool, _>("const o = Object.create(proto); o.me === o")
                .unwrap();

            assert!(got);
        });
    }

    #[test]
    fn define_this_accessor_passes_the_converted_argument_to_write() {
        with_realm(|ctx| {
            define(&ctx);
            define_this_accessor(
                &ctx,
                &ctx.globals().get("proto").unwrap(),
                "slot",
                |_, _| Ok(()),
                |ctx, _, value: i32| ctx.globals().set("got", value),
            )
            .unwrap();

            let got = ctx
                .eval::<i32, _>("Object.create(proto).slot = 40; got")
                .unwrap();

            assert_eq!(got, 40);
        });
    }

    #[test]
    fn define_this_accessor_passes_the_this_value_to_write() {
        with_realm(|ctx| {
            define(&ctx);
            define_this_accessor(
                &ctx,
                &ctx.globals().get("proto").unwrap(),
                "slot",
                |_, _| Ok(()),
                |ctx, this, _: Value| ctx.globals().set("got", this),
            )
            .unwrap();

            let got = ctx
                .eval::<bool, _>("const o = Object.create(proto); o.slot = 1; got === o")
                .unwrap();

            assert!(got);
        });
    }

    #[test]
    fn define_this_method_passes_the_this_value() {
        with_realm(|ctx| {
            define(&ctx);
            define_this_method(
                &ctx,
                &ctx.globals().get("proto").unwrap(),
                "me",
                0,
                |_, this, _| Ok(this),
            )
            .unwrap();

            let got = ctx
                .eval::<bool, _>("const o = Object.create(proto); o.me() === o")
                .unwrap();

            assert!(got);
        });
    }

    #[test]
    fn getter_function_names_the_function_get_key() {
        with_realm(|ctx| {
            let function = getter_function(&ctx, "size", |_, _| Ok(1)).unwrap();

            assert_eq!(function.get::<_, String>("name").unwrap(), "get size");
        });
    }

    #[test]
    fn define_class_gives_the_prototype_the_string_tag() {
        with_realm(|ctx| {
            assert_eq!(
                text(&ctx, "Object.prototype.toString.call(proto)"),
                "[object Widget]"
            );
        });
    }
}
