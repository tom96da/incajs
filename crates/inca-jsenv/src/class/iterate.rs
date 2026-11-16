// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The iterator protocol for native code: the `Symbol.iterator` method of an object and the loop
//! over the values it yields.

use rquickjs::atom::PredefinedAtom;
use rquickjs::function::This;
use rquickjs::{Ctx, Error, Exception, Function, Object, Result as JsResult, Value};

/// The `Symbol.iterator` property of an object.
pub enum Method<'js> {
    /// The property is `undefined` or `null`.
    Absent,
    /// The property is any other value that is no function.
    NotCallable,
    /// The property is a function.
    Function(Function<'js>),
}

/// The `Symbol.iterator` property of `object`.
///
/// # Errors
///
/// Returns the error a getter of the property throws.
pub fn method_of<'js>(object: &Object<'js>) -> JsResult<Method<'js>> {
    let method: Value = object.get(PredefinedAtom::SymbolIterator)?;
    if method.is_undefined() || method.is_null() {
        return Ok(Method::Absent);
    }
    Ok(method
        .into_function()
        .map_or(Method::NotCallable, Method::Function))
}

/// Calls `each` with every value that `method` yields when called on `iterable`. A throw of
/// `each` propagates. When `close` is set, the iterator is closed first.
///
/// # Errors
///
/// Returns the error that `method`, the iterator or `each` throws, and a `TypeError` for a result
/// of `next()` that is no object.
pub fn iterate<'js>(
    ctx: &Ctx<'js>,
    iterable: &Value<'js>,
    method: &Function<'js>,
    close: bool,
    mut each: impl FnMut(Value<'js>) -> JsResult<()>,
) -> JsResult<()> {
    let iterator: Object = method.call((This(iterable.clone()),))?;
    let next: Function = iterator.get("next")?;
    loop {
        let result: Value = next.call((This(iterator.clone()),))?;
        let result = result.into_object().ok_or_else(|| {
            Exception::throw_type(
                ctx,
                "the iterator result is not an object. return an object with done and value",
            )
        })?;
        if result.get::<_, rquickjs::convert::Coerced<bool>>("done")?.0 {
            return Ok(());
        }
        if let Err(error) = each(result.get("value")?) {
            if !close || !matches!(error, Error::Exception) {
                return Err(error);
            }
            let exception = ctx.catch();
            if let Ok(return_method) = iterator.get::<_, Function>("return") {
                let _ = return_method.call::<_, Value>((This(iterator.clone()),));
            }
            drop(ctx.catch());
            return Err(ctx.throw(exception));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rquickjs::Exception;
    use rstest::rstest;

    use super::*;
    use crate::tests::with_realm;

    #[rstest]
    #[case::missing("({})", "absent")]
    #[case::null("({ [Symbol.iterator]: null })", "absent")]
    #[case::undefined("({ [Symbol.iterator]: undefined })", "absent")]
    #[case::number("({ [Symbol.iterator]: 5 })", "not callable")]
    #[case::object("({ [Symbol.iterator]: {} })", "not callable")]
    #[case::function("({ [Symbol.iterator]() {} })", "function")]
    fn method_of_sorts_the_iterator_property(#[case] source: &str, #[case] expected: &str) {
        with_realm(|ctx| {
            let object: Object = ctx.eval(source).unwrap();
            let kind = match method_of(&object).unwrap() {
                Method::Absent => "absent",
                Method::NotCallable => "not callable",
                Method::Function(_) => "function",
            };

            assert_eq!(kind, expected);
        });
    }

    /// Iterates an object that yields 1, 2 and 3 and counts the calls of `return`. `each` throws
    /// at the first value. Returns the thrown message and the count.
    fn failing_loop(close: bool) -> (String, i32) {
        with_realm(|ctx| {
            let iterable: Value = ctx
                .eval(
                    r"
                    globalThis.closed = 0;
                    ({
                        [Symbol.iterator]() {
                            let n = 0;
                            return {
                                next() {
                                    n += 1;
                                    return { done: n > 3, value: n };
                                },
                                return() {
                                    globalThis.closed += 1;
                                    return {};
                                },
                            };
                        },
                    });
                    ",
                )
                .unwrap();
            let method = method_of(iterable.as_object().unwrap()).unwrap();
            let Method::Function(method) = method else {
                unreachable!()
            };
            let result = iterate(&ctx, &iterable, &method, close, |_| {
                Err(Exception::throw_type(&ctx, "stop"))
            });
            assert!(result.is_err());
            let thrown: Object = ctx.catch().into_object().unwrap();

            (
                thrown.get("message").unwrap(),
                ctx.eval("globalThis.closed").unwrap(),
            )
        })
    }

    #[test]
    fn iterate_closes_the_iterator_when_each_throws_with_close() {
        assert_eq!(failing_loop(true), ("stop".to_string(), 1));
    }

    #[test]
    fn iterate_leaves_the_iterator_open_when_each_throws_without_close() {
        assert_eq!(failing_loop(false), ("stop".to_string(), 0));
    }

    #[test]
    fn iterate_throws_a_type_error_for_a_result_that_is_no_object() {
        with_realm(|ctx| {
            let iterable: Value = ctx
                .eval(
                    r"
                    ({
                        [Symbol.iterator]() {
                            return {
                                next() {
                                    return 5;
                                },
                            };
                        },
                    });
                    ",
                )
                .unwrap();
            let method = method_of(iterable.as_object().unwrap()).unwrap();
            let Method::Function(method) = method else {
                unreachable!()
            };
            iterate(&ctx, &iterable, &method, true, |_| Ok(())).unwrap_err();
            let error: Object = ctx.catch().into_object().unwrap();

            assert_eq!(
                (
                    error.get::<_, String>("name").unwrap(),
                    error.get::<_, String>("message").unwrap()
                ),
                (
                    "TypeError".to_string(),
                    "the iterator result is not an object. return an object with done and value"
                        .to_string()
                )
            );
        });
    }

    #[test]
    fn iterate_passes_every_value_to_each() {
        with_realm(|ctx| {
            let iterable: Value = ctx.eval("[10, 20, 30]").unwrap();
            let method = method_of(iterable.as_object().unwrap()).unwrap();
            let Method::Function(method) = method else {
                unreachable!()
            };
            let mut seen = Vec::new();
            iterate(&ctx, &iterable, &method, true, |value| {
                seen.push(value.as_int().unwrap());
                Ok(())
            })
            .unwrap();

            assert_eq!(seen, [10, 20, 30]);
        });
    }
}
