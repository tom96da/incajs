// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The definition steps that every native class shares: its constructor, the
//! methods and getters of its prototype and its global.
//!
//! Methods and accessors are enumerable. The global is writable, configurable
//! and not enumerable.

use rquickjs::atom::PredefinedAtom;
use rquickjs::function::{IntoJsFunc, Params};
use rquickjs::object::Property;
use rquickjs::{Ctx, Function, Object, Result as JsResult, Value};

use crate::FunctionAccessor;

/// The `TypeError` message of a class constructor called without `new`.
pub(crate) const NEEDS_NEW: &str = "the class constructor needs new. call it with new";

/// Emits the empty `Trace` and the `JsClass` of a class whose instances hold plain Rust data and
/// take their prototype from the vault.
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

pub(crate) use plain_class;

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

/// The prototype for an instance a constructor builds: the `prototype` of the constructor that
/// `new` named, so a subclass keeps its own, and `default` when that is no object.
///
/// # Errors
///
/// Returns an error if reading `prototype` throws.
pub fn new_target_prototype<'js>(
    params: &Params<'_, 'js>,
    default: Object<'js>,
) -> JsResult<Object<'js>> {
    let proto = params
        .this()
        .as_object()
        .map(|target| target.get::<_, Value>("prototype"))
        .transpose()?
        .and_then(Value::into_object);
    Ok(proto.unwrap_or(default))
}

/// Defines `function` as the writable, enumerable, configurable method `name` of `proto`.
///
/// # Errors
///
/// Returns an error if the property cannot be defined.
pub fn define_method<'js>(
    proto: &Object<'js>,
    name: &str,
    function: Function<'js>,
) -> JsResult<()> {
    proto.prop(
        name,
        Property::from(function.with_name(name)?)
            .writable()
            .enumerable()
            .configurable(),
    )
}

/// Defines `get` as the enumerable, configurable getter `key` of `proto`, without a setter. The
/// function is named `get <key>`.
///
/// # Errors
///
/// Returns an error if the property cannot be defined.
pub fn define_getter<'js>(proto: &Object<'js>, key: &str, get: Function<'js>) -> JsResult<()> {
    proto.prop(
        key,
        FunctionAccessor::new_get(get.with_name(format!("get {key}"))?)
            .enumerable()
            .configurable(),
    )
}

/// Defines `get` and `set` as the enumerable, configurable accessor `key` of `proto`. The
/// functions are named `get <key>` and `set <key>`.
///
/// # Errors
///
/// Returns an error if the property cannot be defined.
pub fn define_accessor<'js>(
    proto: &Object<'js>,
    key: &str,
    get: Function<'js>,
    set: Function<'js>,
) -> JsResult<()> {
    proto.prop(
        key,
        FunctionAccessor::new_get(get.with_name(format!("get {key}"))?)
            .set(set.with_name(format!("set {key}"))?)
            .enumerable()
            .configurable(),
    )
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rquickjs::function::{IntoJsFunc, ParamRequirement, Params};
    use rquickjs::{Ctx, Function, Object, Result as JsResult, Value};
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
        define_global(ctx, "Widget", widget).unwrap();
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
    fn define_class_gives_the_prototype_the_string_tag() {
        with_realm(|ctx| {
            assert_eq!(
                text(&ctx, "Object.prototype.toString.call(proto)"),
                "[object Widget]"
            );
        });
    }

    /// A constructor body that builds an object with the prototype `new_target_prototype` picks,
    /// with `Widget.prototype` as the default.
    struct Build;

    impl<'js> IntoJsFunc<'js, ()> for Build {
        fn param_requirements() -> ParamRequirement {
            ParamRequirement::any()
        }

        fn call<'a>(&self, params: Params<'a, 'js>) -> JsResult<Value<'js>> {
            let object = Object::new(params.ctx().clone())?;
            let widget: Object = params.ctx().globals().get("Widget")?;
            let default: Object = widget.get("prototype")?;
            object.set_prototype(Some(&new_target_prototype(&params, default)?))?;
            Ok(object.into_value())
        }
    }

    #[rstest]
    #[case::plain("String(new Widget() instanceof Widget)", "true")]
    #[case::subclass_instance(
        r"
        class Sub extends Widget {}
        String(Object.getPrototypeOf(new Sub()) === Sub.prototype)
        ",
        "true"
    )]
    #[case::subclass_prototype(
        r"
        class Sub extends Widget {
            hello() {
                return 'hi';
            }
        }
        new Sub().hello()
        ",
        "hi"
    )]
    fn new_target_prototype_takes_the_prototype_of_new_target(
        #[case] source: &str,
        #[case] expected: &str,
    ) {
        with_realm(|ctx| {
            let proto = Object::new(ctx.clone()).unwrap();
            let widget = define_class(&ctx, &proto, "Widget", 0, Build).unwrap();
            define_global(&ctx, "Widget", widget).unwrap();

            assert_eq!(ctx.eval::<String, _>(source).unwrap(), expected);
        });
    }

    #[rstest]
    #[case::null_prototype(
        r"
        const target = function () {};
        target.prototype = null;
        const r = Reflect.construct(Widget, [], target);
        "
    )]
    #[case::plain_call("const r = Widget();")]
    fn new_target_prototype_falls_back_to_the_default(#[case] source: &str) {
        with_realm(|ctx| {
            let proto = Object::new(ctx.clone()).unwrap();
            let widget = define_class(&ctx, &proto, "Widget", 0, Build).unwrap();
            define_global(&ctx, "Widget", widget).unwrap();
            let script = format!("{source}\nString(Object.getPrototypeOf(r) === Widget.prototype)");

            assert_eq!(ctx.eval::<String, _>(script).unwrap(), "true");
        });
    }
}
