// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The clones of an `Error` and of a `DOMException`.

use rquickjs::convert::Coerced;
use rquickjs::function::This;
use rquickjs::object::Property;
use rquickjs::{FromJs, Object, Result as JsResult, String as JsString, Value};

use crate::quickjs::ObjectExt;
use crate::structured_clone::intrinsics::ERRORS;
use crate::structured_clone::walk::{Frame, Walk};

impl<'js> Walk<'js> {
    /// The clone of an `Error`: its `name` picks the constructor, its own `message`, `stack` and
    /// `cause` follow. The cause is cloned by the loop.
    pub(super) fn clone_error(&mut self, source: &Object<'js>) -> JsResult<Value<'js>> {
        let value = source.as_value();
        let name: Value = source.get("name")?;
        let name = name
            .as_string()
            .and_then(|name| name.to_string().ok())
            .filter(|name| ERRORS.contains(&name.as_str()))
            .unwrap_or_else(|| "Error".to_string());
        let message = match source.get_own_property_descriptor("message")? {
            Some(descriptor) if !descriptor.is_accessor() => {
                Some(Coerced::<JsString>::from_js(&self.ctx, descriptor.value)?.0)
            }
            _ => None,
        };
        let clone = match message {
            Some(message) => self.tk.construct_member("errors", &name, (message,))?,
            None => self.tk.construct_member("errors", &name, ())?,
        };
        let stack = match source.get_own_property_descriptor("stack")? {
            Some(descriptor) if !descriptor.is_accessor() => descriptor.value,
            Some(_) => source.get("stack")?,
            None => self.tk.call("errorStack", (This(value.clone()),))?,
        };
        if stack.is_string() {
            clone.prop("stack", Property::from(stack).writable().configurable())?;
        }
        let clone_value = clone.clone().into_value();
        self.remember(value, &clone_value)?;
        if let Some(descriptor) = source.get_own_property_descriptor("cause")?
            && !descriptor.is_accessor()
        {
            self.frames.push(Frame::Cause {
                target: clone,
                cause: descriptor.value,
            });
        }
        Ok(clone_value)
    }

    /// The clone of a `DOMException`: its name and message, and the stack the engine gives it.
    pub(super) fn clone_dom_exception(&self, value: &Value<'js>) -> JsResult<Value<'js>> {
        let name: Value = self.tk.call("domName", (This(value.clone()),))?;
        let message: Value = self.tk.call("domMessage", (This(value.clone()),))?;
        let clone: Object = self.tk.construct("DOMException", (message, name))?;
        Ok(clone.into_value())
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::tests::{run, thrown};

    #[test]
    fn a_listed_own_name_keeps_that_error_prototype() {
        assert_eq!(
            run(r"
                const e = new Error('m');
                e.name = 'TypeError';
                String(Object.getPrototypeOf(structuredClone(e)) === TypeError.prototype);
                "),
            "true"
        );
    }

    #[rstest]
    #[case::unlisted("e.name = 'Foo';")]
    #[case::not_a_string("e.name = 5;")]
    #[case::subclass("e = new (class extends Error {})('m');")]
    fn another_name_gives_a_plain_error(#[case] setup: &str) {
        assert_eq!(
            run(&format!(
                r"
                let e = new Error('m');
                {setup};
                String(Object.getPrototypeOf(structuredClone(e)) === Error.prototype);
                "
            )),
            "true"
        );
    }

    #[test]
    fn an_aggregate_error_gives_an_error_with_no_errors() {
        assert_eq!(
            run(r"
                const c = structuredClone(new AggregateError([new Error('a')], 'm'));
                `${Object.getPrototypeOf(c) === Error.prototype}:${'errors' in c}`;
                "),
            "true:false"
        );
    }

    #[test]
    fn an_own_name_getter_that_throws_surfaces_its_value() {
        assert_eq!(
            thrown(
                r"
                const e = new Error('x');
                Object.defineProperty(e, 'name', {
                    get() {
                        throw new RangeError('boom');
                    },
                });
                structuredClone(e);
                "
            ),
            "RangeError: boom"
        );
    }

    #[test]
    fn an_own_message_is_a_non_enumerable_writable_configurable_property() {
        assert_eq!(
            run(r"
                const d = Object.getOwnPropertyDescriptor(structuredClone(new Error('hi')), 'message');
                [d.value, d.enumerable, d.writable, d.configurable].join();
                "),
            "hi,false,true,true"
        );
    }

    #[rstest]
    #[case::number("5", "5")]
    #[case::undefined("undefined", "undefined")]
    fn a_message_that_is_no_string_becomes_a_string(#[case] message: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!(
                r"
                const e = new Error('m');
                e.message = {message};
                structuredClone(e).message;
                "
            )),
            expected
        );
    }

    #[test]
    fn no_own_message_gives_a_clone_with_no_own_message() {
        assert_eq!(
            run("String(Object.hasOwn(structuredClone(new Error()), 'message'))"),
            "false"
        );
    }

    #[test]
    fn an_accessor_message_is_not_read() {
        assert_eq!(
            run(r"
                let reads = 0;
                const e = new Error('m');
                Object.defineProperty(e, 'message', {
                    get() {
                        reads += 1;
                        return 'x';
                    },
                });
                const c = structuredClone(e);
                `${reads}:${Object.hasOwn(c, 'message')}`;
                "),
            "0:false"
        );
    }

    #[test]
    fn a_message_whose_to_string_throws_surfaces_its_value() {
        assert_eq!(
            thrown(
                r"
                const e = new Error('m');
                e.message = {
                    toString() {
                        throw new RangeError('boom');
                    },
                };
                structuredClone(e);
                "
            ),
            "RangeError: boom"
        );
    }

    #[test]
    fn a_symbol_message_throws_a_type_error() {
        assert_eq!(
            run(r"
                const e = new Error('m');
                e.message = Symbol();
                try {
                    structuredClone(e);
                } catch (error) {
                    error.name;
                }
                "),
            "TypeError"
        );
    }

    #[test]
    fn an_error_with_no_own_stack_takes_the_stack_of_the_source() {
        assert_eq!(
            run(r"
                const e = new Error('m');
                String(structuredClone(e).stack === e.stack);
                "),
            "true"
        );
    }

    #[test]
    fn an_own_stack_string_is_copied_as_a_non_enumerable_property() {
        assert_eq!(
            run(r"
                const e = new Error('m');
                Object.defineProperty(e, 'stack', { value: 'custom', configurable: true });
                const d = Object.getOwnPropertyDescriptor(structuredClone(e), 'stack');
                [d.value, d.enumerable, d.writable, d.configurable].join();
                "),
            "custom,false,true,true"
        );
    }

    #[test]
    fn an_own_stack_accessor_runs_once_and_its_string_is_copied() {
        assert_eq!(
            run(r"
                let reads = 0;
                const e = new Error('m');
                Object.defineProperty(e, 'stack', {
                    get() {
                        reads += 1;
                        return 'from getter';
                    },
                    configurable: true,
                });
                const c = structuredClone(e);
                `${Object.getOwnPropertyDescriptor(c, 'stack').value}:${reads}`;
                "),
            "from getter:1"
        );
    }

    #[rstest]
    #[case::accessor(
        "Object.defineProperty(e, 'stack', { get() { return 5; }, configurable: true });"
    )]
    #[case::data("Object.defineProperty(e, 'stack', { value: 5, configurable: true });")]
    fn a_stack_that_is_no_string_leaves_the_stack_of_the_clone(#[case] setup: &str) {
        assert_eq!(
            run(&format!(
                r"
                const e = new Error('m');
                {setup};
                typeof structuredClone(e).stack;
                "
            )),
            "string"
        );
    }

    #[test]
    fn a_data_cause_is_cloned_as_a_non_enumerable_property() {
        assert_eq!(
            run(r"
                const e = new Error('m');
                e.cause = { a: 1 };
                const c = structuredClone(e);
                const d = Object.getOwnPropertyDescriptor(c, 'cause');
                [d.enumerable, d.writable, d.configurable, c.cause !== e.cause, c.cause.a].join();
                "),
            "false,true,true,true,1"
        );
    }

    #[test]
    fn an_accessor_cause_is_not_read() {
        assert_eq!(
            run(r"
                let reads = 0;
                const e = new Error('m');
                Object.defineProperty(e, 'cause', {
                    get() {
                        reads += 1;
                        return 1;
                    },
                });
                const c = structuredClone(e);
                `${reads}:${Object.hasOwn(c, 'cause')}`;
                "),
            "0:false"
        );
    }

    #[test]
    fn other_own_properties_of_an_error_are_dropped() {
        assert_eq!(
            run(r"
                const e = new Error('m');
                e.code = 1;
                e.extra = 2;
                const c = structuredClone(e);
                `${'code' in c}:${'extra' in c}`;
                "),
            "false:false"
        );
    }

    #[test]
    fn a_hook_that_returns_no_string_keeps_the_stack_of_the_source() {
        assert_eq!(
            run(r"
                const e = new Error('m');
                const original = e.stack;
                Error.prepareStackTrace = () => 5;
                String(structuredClone(e).stack === original);
                "),
            "true"
        );
    }

    #[rstest]
    #[case::name("c.name", "AbortError")]
    #[case::message("c.message", "m")]
    #[case::code("String(c.code)", "20")]
    #[case::instance("String(c instanceof DOMException)", "true")]
    #[case::not_the_source("String(c !== d)", "true")]
    fn a_dom_exception_clone_keeps_the_exception(#[case] read: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!(
                r"
                const d = new DOMException('m', 'AbortError');
                const c = structuredClone(d);
                {read};
                "
            )),
            expected
        );
    }

    #[test]
    fn an_own_stack_string_of_a_dom_exception_is_not_copied() {
        assert_eq!(
            run(r"
                const d = new DOMException('m', 'AbortError');
                d.stack = 'custom';
                String(structuredClone(d).stack === 'custom');
                "),
            "false"
        );
    }

    #[test]
    fn an_own_stack_accessor_of_a_dom_exception_runs_no_getter() {
        assert_eq!(
            run(r"
                let reads = 0;
                const d = new DOMException('m', 'AbortError');
                Object.defineProperty(d, 'stack', {
                    get() {
                        reads += 1;
                        return 'x';
                    },
                    configurable: true,
                });
                structuredClone(d);
                String(reads);
                "),
            "0"
        );
    }

    #[test]
    fn a_number_stack_of_a_dom_exception_is_not_copied() {
        assert_eq!(
            run(r"
                const d = new DOMException('m', 'AbortError');
                d.stack = 5;
                typeof structuredClone(d).stack;
                "),
            "string"
        );
    }

    #[test]
    fn a_hook_that_returns_five_leaves_five_as_the_stack_of_the_dom_exception_clone() {
        assert_eq!(
            run(r"
                const d = new DOMException('m', 'AbortError');
                Object.defineProperty(d, 'stack', {
                    get() {
                        return 'accessor';
                    },
                    configurable: true,
                });
                Error.prepareStackTrace = () => 5;
                String(structuredClone(d).stack);
                "),
            "5"
        );
    }

    #[test]
    fn a_replaced_error_global_changes_no_error_clone() {
        assert_eq!(
            run(r"
                const proto = TypeError.prototype;
                const source = new TypeError('m');
                globalThis.TypeError = function () {
                    throw new Error('used');
                };
                globalThis.Error = function () {
                    throw new Error('used');
                };
                String(Object.getPrototypeOf(structuredClone(source)) === proto);
                "),
            "true"
        );
    }

    #[test]
    fn a_replaced_dom_exception_global_changes_no_clone() {
        assert_eq!(
            run(r"
                const proto = DOMException.prototype;
                const source = new DOMException('m', 'AbortError');
                globalThis.DOMException = function () {
                    throw new Error('used');
                };
                String(Object.getPrototypeOf(structuredClone(source)) === proto);
                "),
            "true"
        );
    }

    #[test]
    fn replaced_name_and_message_getters_change_no_dom_exception_clone() {
        assert_eq!(
            run(r"
                const name = Object.getOwnPropertyDescriptor(DOMException.prototype, 'name').get;
                const message = Object.getOwnPropertyDescriptor(DOMException.prototype, 'message').get;
                const source = new DOMException('m', 'AbortError');
                Object.defineProperty(DOMException.prototype, 'name', {
                    get() {
                        return 'X';
                    },
                });
                Object.defineProperty(DOMException.prototype, 'message', {
                    get() {
                        return 'Y';
                    },
                });
                const c = structuredClone(source);
                `${name.call(c)}:${message.call(c)}`;
                "),
            "AbortError:m"
        );
    }

    #[test]
    fn a_replaced_stack_accessor_changes_no_error_clone() {
        assert_eq!(
            run(r"
                const original = Object.getOwnPropertyDescriptor(Error.prototype, 'stack').get;
                const source = new Error('m');
                Object.defineProperty(Error.prototype, 'stack', {
                    get() {
                        return 'x';
                    },
                    configurable: true,
                });
                const c = structuredClone(source);
                String(Object.getOwnPropertyDescriptor(c, 'stack').value === original.call(source));
                "),
            "true"
        );
    }
}
