// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The checks a native function makes when a script calls it: that a class constructor got `new`,
//! that a method got an instance of its class, that enough arguments came and which prototype a
//! new instance takes.

use rquickjs::class::JsClass;
use rquickjs::convert::Coerced;
use rquickjs::function::Params;
use rquickjs::{Class, Ctx, Exception, FromJs, Object, Result as JsResult, Value};

/// The `TypeError` message of a class constructor called without `new`.
pub(crate) const NEEDS_NEW: &str = "the class constructor needs new. call it with new";

/// The body of the constructors that always throw.
pub(crate) fn illegal_constructor<'js>() -> impl Fn(Ctx<'js>) -> JsResult<Value<'js>> {
    |ctx| Err(Exception::throw_type(&ctx, "illegal constructor"))
}

/// The class instance `this` is, or a `TypeError` that names the class.
///
/// # Errors
///
/// Returns a `TypeError` if `this` is not an instance of `T`.
pub fn receiver<'js, T: JsClass<'js>>(
    ctx: &Ctx<'js>,
    this: &Value<'js>,
) -> JsResult<Class<'js, T>> {
    Class::<T>::from_value(this).map_err(|_| {
        let message = format!(
            "the receiver is not a {}. call the method on an instance",
            T::NAME
        );
        Exception::throw_type(ctx, &message)
    })
}

/// `value` converted with `ToString`.
///
/// # Errors
///
/// Returns the error `ToString` throws, as for a `Symbol`.
pub fn text<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<String> {
    Ok(Coerced::<String>::from_js(ctx, value.clone())?.0)
}

/// A `TypeError` unless `args` holds `count` arguments.
///
/// # Errors
///
/// Returns a `TypeError` if `args` holds fewer than `count` arguments.
pub fn require(ctx: &Ctx<'_>, method: &str, args: &[Value<'_>], count: usize) -> JsResult<()> {
    if args.len() < count {
        let message = format!("too few arguments for {method}. pass at least {count}");
        return Err(Exception::throw_type(ctx, &message));
    }
    Ok(())
}

/// The argument at `index`, `None` when it is absent or `undefined`.
pub(crate) fn given<'a, 'js>(args: &'a [Value<'js>], index: usize) -> Option<&'a Value<'js>> {
    args.get(index).filter(|value| !value.is_undefined())
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rquickjs::function::{IntoJsFunc, ParamRequirement, Params};
    use rquickjs::{Object, Result as JsResult, Value};
    use rstest::rstest;

    use super::*;
    use crate::class::{define_class, define_global};
    use crate::tests::with_realm;

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
