// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The global function `structuredClone`: its receiver, its arguments and the errors it throws.

use rquickjs::function::{Rest, This};
use rquickjs::{Ctx, Error, Exception, Function, Result as JsResult, Value};

use super::buffer::check_transfer_list;
use super::intrinsics;
use super::walk::clone_value;
use crate::class::{Method, define_function, iterate, method_of, options_of, require};

const RECEIVER: &str = concat!(
    "structuredClone is called on a value that is not the global object. ",
    "call it as a plain function"
);
const NOT_ITERABLE: &str =
    "the transfer list is not iterable. pass an array of ArrayBuffer objects";
const NOT_AN_OBJECT_ITEM: &str =
    "the transfer list holds a value that is not an object. list ArrayBuffer objects only";

/// What a value or a transfer list holds that cannot be cloned or moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Cause {
    Symbol,
    Function,
    Proxy,
    Object,
    Shared,
    Detached,
    ViewOutOfBounds,
    TransferItem,
    TransferTwice,
    TransferDetached,
    TransferImmutable,
}

/// Throws the error for `cause`: a `DataCloneError`, or a `TypeError` for an immutable item.
pub(super) fn refuse(ctx: &Ctx<'_>, cause: Cause) -> Error {
    let message = match cause {
        Cause::Symbol => "a symbol cannot be cloned. remove it from the value",
        Cause::Function => "a function cannot be cloned. remove it from the value",
        Cause::Proxy => "a proxy cannot be cloned. clone its target",
        Cause::Object => "the object cannot be cloned. pass plain data",
        Cause::Shared => "a shared buffer cannot be cloned. copy its bytes into an ArrayBuffer",
        Cause::Detached => "the buffer is detached. clone a buffer that holds data",
        Cause::ViewOutOfBounds => "the view is out of bounds. clone a view inside its buffer",
        Cause::TransferItem => concat!(
            "the transfer list holds a value that cannot be transferred. ",
            "list ArrayBuffer objects only"
        ),
        Cause::TransferTwice => "the transfer list holds one buffer twice. list each buffer once",
        Cause::TransferDetached => {
            "a buffer in the transfer list is detached. list buffers that hold data"
        }
        Cause::TransferImmutable => {
            let message =
                "a buffer in the transfer list is immutable. list buffers that can be detached";
            return Exception::throw_type(ctx, message);
        }
    };
    Exception::throw_dom(ctx, "DataCloneError", message)
}

/// Defines the global function `structuredClone`.
///
/// # Errors
///
/// Returns an error if the realm lacks an intrinsic, has no vault or a global cannot be defined.
pub(crate) fn install<'js>(ctx: &Ctx<'js>) -> JsResult<()> {
    intrinsics::install(ctx)?;
    let function = Function::new(
        ctx.clone(),
        |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            structured_clone(&ctx, &this, &args)
        },
    )?
    .with_length(1)?;
    define_function(ctx, "structuredClone", function)
}

/// The body of `structuredClone`.
fn structured_clone<'js>(
    ctx: &Ctx<'js>,
    this: &Value<'js>,
    args: &[Value<'js>],
) -> JsResult<Value<'js>> {
    let global = ctx.globals().into_value();
    if !(this.is_undefined() || this.is_null() || *this == global) {
        return Err(Exception::throw_type(ctx, RECEIVER));
    }
    require(ctx, "structuredClone", args, 1)?;
    let options = options_of(ctx, args.get(1).cloned())?;
    let items = match options {
        Some(options) => transfer_list(ctx, &options.get("transfer")?)?,
        None => Vec::new(),
    };
    let transfers = check_transfer_list(ctx, items)?;
    clone_value(ctx, &args[0], &transfers)
}

/// The objects of the `transfer` member, as a Web IDL `sequence<object>`.
fn transfer_list<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<Vec<Value<'js>>> {
    if value.is_undefined() {
        return Ok(Vec::new());
    }
    let object = value
        .as_object()
        .ok_or_else(|| Exception::throw_type(ctx, NOT_ITERABLE))?;
    let Method::Function(method) = method_of(object)? else {
        return Err(Exception::throw_type(ctx, NOT_ITERABLE));
    };
    let mut items = Vec::new();
    iterate(ctx, value, &method, false, |item| {
        if !item.is_object() {
            return Err(Exception::throw_type(ctx, NOT_AN_OBJECT_ITEM));
        }
        items.push(item);
        Ok(())
    })?;
    Ok(items)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::class::NOT_AN_OBJECT;
    use crate::tests::{run, thrown};

    const NOT_ITERABLE_TEXT: &str =
        "TypeError: the transfer list is not iterable. pass an array of ArrayBuffer objects";
    const ITEM_TEXT: &str = concat!(
        "DataCloneError: the transfer list holds a value that cannot be transferred. ",
        "list ArrayBuffer objects only"
    );
    const TWICE_TEXT: &str =
        "DataCloneError: the transfer list holds one buffer twice. list each buffer once";
    const DETACHED_ITEM_TEXT: &str =
        "DataCloneError: a buffer in the transfer list is detached. list buffers that hold data";
    const IMMUTABLE_TEXT: &str = concat!(
        "TypeError: a buffer in the transfer list is immutable. ",
        "list buffers that can be detached"
    );
    const RECEIVER_TEXT: &str = concat!(
        "TypeError: structuredClone is called on a value that is not the global object. ",
        "call it as a plain function"
    );

    #[test]
    fn the_global_is_a_writable_enumerable_configurable_function() {
        assert_eq!(
            run(r"
                const d = Object.getOwnPropertyDescriptor(globalThis, 'structuredClone');
                [typeof d.value, d.writable, d.enumerable, d.configurable].join();
                "),
            "function,true,true,true"
        );
    }

    #[test]
    fn the_function_is_named_structured_clone() {
        assert_eq!(run("structuredClone.name"), "structuredClone");
    }

    #[test]
    fn the_function_has_length_one() {
        assert_eq!(run("String(structuredClone.length)"), "1");
    }

    #[test]
    fn calling_without_an_argument_throws() {
        assert_eq!(
            thrown("structuredClone()"),
            "TypeError: too few arguments for structuredClone. pass at least 1"
        );
    }

    #[test]
    fn options_that_are_no_object_throw() {
        assert_eq!(
            thrown("structuredClone(1, 'a')"),
            format!("TypeError: {NOT_AN_OBJECT}")
        );
    }

    #[rstest]
    #[case::undefined("undefined")]
    #[case::null("null")]
    #[case::object("{}")]
    fn options_that_are_empty_or_an_object_are_accepted(#[case] options: &str) {
        assert_eq!(run(&format!("String(structuredClone(1, {options}))")), "1");
    }

    #[rstest]
    #[case::length_object("{ length: 0 }")]
    #[case::null("null")]
    #[case::string("'ab'")]
    fn a_transfer_list_that_is_not_iterable_throws(#[case] list: &str) {
        assert_eq!(
            thrown(&format!("structuredClone(1, {{ transfer: {list} }})")),
            NOT_ITERABLE_TEXT
        );
    }

    #[test]
    fn a_transfer_list_may_be_any_iterable() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4);
                structuredClone(1, { transfer: new Set([b]) });
                String(b.detached);
                "),
            "true"
        );
    }

    #[test]
    fn a_transfer_item_that_is_no_object_throws() {
        assert_eq!(
            thrown("structuredClone(1, { transfer: [1] })"),
            "TypeError: the transfer list holds a value that is not an object. list ArrayBuffer objects only"
        );
    }

    #[test]
    fn transfer_is_read_once() {
        assert_eq!(
            run(r"
                let reads = 0;
                structuredClone(1, {
                    get transfer() {
                        reads += 1;
                        return [];
                    },
                });
                String(reads);
                "),
            "1"
        );
    }

    #[test]
    fn transfer_is_read_before_a_getter_of_the_value() {
        assert_eq!(
            run(r"
                const log = [];
                const value = {
                    get a() {
                        log.push('value');
                        return 1;
                    },
                };
                structuredClone(value, {
                    get transfer() {
                        log.push('transfer');
                        return [];
                    },
                });
                log.join();
                "),
            "transfer,value"
        );
    }

    #[test]
    fn a_transfer_getter_that_throws_surfaces_its_error() {
        assert_eq!(
            thrown(
                r"
                structuredClone(1, {
                    get transfer() {
                        throw new RangeError('boom');
                    },
                })
                "
            ),
            "RangeError: boom"
        );
    }

    #[test]
    fn an_iterator_that_throws_surfaces_its_error() {
        assert_eq!(
            thrown(
                r"
                structuredClone(1, {
                    transfer: {
                        [Symbol.iterator]() {
                            throw new RangeError('boom');
                        },
                    },
                })
                "
            ),
            "RangeError: boom"
        );
    }

    #[test]
    fn an_iterator_whose_item_is_no_object_is_not_closed() {
        assert_eq!(
            run(r"
                let closed = 0;
                const transfer = {
                    [Symbol.iterator]() {
                        return {
                            next() {
                                return { done: false, value: 1 };
                            },
                            return() {
                                closed += 1;
                                return {};
                            },
                        };
                    },
                };
                try {
                    structuredClone(1, { transfer });
                } catch (e) {}
                String(closed);
                "),
            "0"
        );
    }

    #[rstest]
    #[case::undefined("String(structuredClone.call(undefined, 1))")]
    #[case::null("String(structuredClone.call(null, 1))")]
    #[case::global("String(structuredClone.call(globalThis, 1))")]
    #[case::extracted(
        r"
        const f = structuredClone;
        String(f(1));
        "
    )]
    fn a_receiver_that_is_missing_or_global_is_accepted(#[case] source: &str) {
        assert_eq!(run(source), "1");
    }

    #[rstest]
    #[case::object("{}")]
    #[case::number("5")]
    fn another_receiver_throws(#[case] receiver: &str) {
        assert_eq!(
            thrown(&format!("structuredClone.call({receiver}, 1)")),
            RECEIVER_TEXT
        );
    }

    #[test]
    fn a_data_clone_error_is_a_dom_exception_with_code_25() {
        assert_eq!(
            run(r"
                try {
                    structuredClone(() => {});
                } catch (e) {
                    [e instanceof DOMException, e.name, e.code].join();
                }
                "),
            "true,DataCloneError,25"
        );
    }

    #[test]
    fn a_primitive_with_a_transfer_list_still_moves_the_buffer() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4);
                const r = structuredClone(1, { transfer: [b] });
                `${r} ${b.detached}`;
                "),
            "1 true"
        );
    }

    #[test]
    fn a_primitive_with_a_twice_listed_buffer_throws() {
        assert_eq!(
            thrown(
                r"
                const b = new ArrayBuffer(4);
                structuredClone(1, { transfer: [b, b] })
                "
            ),
            TWICE_TEXT
        );
    }

    #[test]
    fn the_list_is_checked_before_the_walk() {
        assert_eq!(
            run(r"
                const log = [];
                const value = {
                    get a() {
                        log.push('walk');
                        return 1;
                    },
                };
                let text;
                try {
                    structuredClone(value, { transfer: [{}] });
                } catch (e) {
                    text = `${e.name}: ${e.message}`;
                }
                `${text}|${log.join()}`;
                "),
            format!("{ITEM_TEXT}|")
        );
    }

    #[test]
    fn a_symbol_with_an_invalid_list_gives_the_transfer_message() {
        assert_eq!(
            thrown("structuredClone(Symbol(), { transfer: [{}] })"),
            ITEM_TEXT
        );
    }

    #[test]
    fn a_walk_that_throws_leaves_a_listed_buffer_attached() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4);
                try {
                    structuredClone(
                        {
                            get a() {
                                throw new Error('x');
                            },
                        },
                        { transfer: [b] },
                    );
                } catch (e) {}
                String(b.detached);
                "),
            "false"
        );
    }

    #[test]
    fn a_getter_that_detaches_a_listed_buffer_gives_the_detached_item_error() {
        assert_eq!(
            thrown(
                r"
                const b = new ArrayBuffer(4);
                structuredClone(
                    {
                        b,
                        get g() {
                            b.transfer();
                            return 1;
                        },
                    },
                    { transfer: [b] },
                );
                "
            ),
            DETACHED_ITEM_TEXT
        );
    }

    #[test]
    fn buffers_before_a_failing_one_stay_detached() {
        assert_eq!(
            run(r"
                const b1 = new ArrayBuffer(4);
                const b2 = new ArrayBuffer(4);
                try {
                    structuredClone(
                        {
                            b1,
                            b2,
                            get g() {
                                b2.transfer();
                                return 1;
                            },
                        },
                        { transfer: [b1, b2] },
                    );
                } catch (e) {}
                String(b1.detached);
                "),
            "true"
        );
    }

    #[test]
    fn an_immutable_buffer_in_the_list_throws_after_the_walk() {
        assert_eq!(
            run(r"
                const log = [];
                const b = new ArrayBuffer(4).transferToImmutable();
                let text;
                try {
                    structuredClone(
                        {
                            get g() {
                                log.push('walk');
                                return 1;
                            },
                        },
                        { transfer: [b] },
                    );
                } catch (e) {
                    text = `${e.name}: ${e.message}`;
                }
                `${text}|${log.join()}`;
                "),
            format!("{IMMUTABLE_TEXT}|walk")
        );
    }

    #[test]
    fn a_buffer_detached_before_the_call_runs_the_getters_then_throws() {
        assert_eq!(
            run(r"
                const log = [];
                const b = new ArrayBuffer(4);
                b.transfer();
                let text;
                try {
                    structuredClone(
                        {
                            b,
                            get g() {
                                log.push('getter');
                                return 1;
                            },
                        },
                        { transfer: [b] },
                    );
                } catch (e) {
                    text = `${e.name}: ${e.message}`;
                }
                `${text}|${log.join()}`;
                "),
            format!("{DETACHED_ITEM_TEXT}|getter")
        );
    }
}
