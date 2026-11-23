// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The values with no children: boxed primitives, `Date` and `RegExp`. Each clone is built from
//! the intrinsics captured before any script ran.

use rquickjs::function::This;
use rquickjs::{Function, Result as JsResult, Value};

use super::intrinsics::Kind;
use super::walk::Walk;

/// The order of the flags of a `RegExp`.
const FLAGS: [&str; 8] = ["d", "g", "i", "m", "s", "u", "v", "y"];

impl<'js> Walk<'js> {
    /// The clone of a `DOMRect` or a `DOMRectReadOnly`, an instance of the same interface.
    pub(super) fn clone_rect(&self, value: &Value<'js>) -> JsResult<Value<'js>> {
        crate::geometry::copy(&self.ctx, value)
    }

    /// The clone of a boxed primitive, a `Date` or a `RegExp`.
    pub(super) fn clone_leaf(&self, kind: Kind, value: &Value<'js>) -> JsResult<Value<'js>> {
        match kind {
            Kind::Boxed => {
                let key = self.tk.value_of_key(value);
                let primitive: Value = self.tk.call(key, (This(value.clone()),))?;
                let object: Function = self.tk.get("Object")?;
                object.call((primitive,))
            }
            Kind::Date => {
                let time: f64 = self.tk.call("dateGetTime", (This(value.clone()),))?;
                Ok(self.tk.construct("Date", (time,))?.into_value())
            }
            _ => {
                let source: Value = self.tk.call("regexpSource", (This(value.clone()),))?;
                let mut flags = String::new();
                for flag in FLAGS {
                    let set: bool = self.tk.call_member("regexpFlags", flag, value)?;
                    if set {
                        flags.push_str(flag);
                    }
                }
                Ok(self.tk.construct("RegExp", (source, flags))?.into_value())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::tests::run;

    #[rstest]
    #[case::number("Object(5)", "Number,5")]
    #[case::string("Object('s')", "String,s")]
    #[case::boolean("Object(true)", "Boolean,true")]
    #[case::bigint("Object(5n)", "BigInt,5")]
    fn a_boxed_primitive_gives_a_new_object_of_the_same_kind_and_value(
        #[case] source: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            run(&format!(
                r"
                const source = {source};
                const c = structuredClone(source);
                const kind = Object.prototype.toString.call(c).slice(8, -1);
                `${{kind}},${{c.valueOf()}}:${{typeof c}}:${{c !== source}}`;
                "
            )),
            format!("{expected}:object:true")
        );
    }

    #[rstest]
    #[case::writable("DOMRect", "DOMRect")]
    #[case::read_only("DOMRectReadOnly", "DOMRectReadOnly")]
    fn a_rect_gives_a_new_rect_of_the_same_interface(#[case] class: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!(
                r"
                const source = new {class}(1, 2, 3, 4);
                const c = structuredClone(source);
                const same = Object.getPrototypeOf(c) === {class}.prototype;
                const values = [c.x, c.y, c.width, c.height].join();
                `${{same}}:${{c !== source}}:${{values}}:${{c[Symbol.toStringTag]}}`;
                "
            )),
            format!("true:true:1,2,3,4:{expected}")
        );
    }

    #[test]
    fn a_rect_referenced_twice_is_cloned_once() {
        assert_eq!(
            run(r"
                const r = new DOMRect(1, 2, 3, 4);
                const c = structuredClone([r, r]);
                `${c[0] === c[1]}${c[0] !== r}`;
                "),
            "truetrue"
        );
    }

    #[test]
    fn an_own_value_of_on_a_boxed_primitive_is_not_called() {
        assert_eq!(
            run(r"
                const source = Object(5);
                source.valueOf = () => {
                    throw new Error('called');
                };
                String(structuredClone(source).valueOf());
                "),
            "5"
        );
    }

    #[rstest]
    #[case::time("5")]
    #[case::invalid("NaN")]
    #[case::max("8.64e15")]
    #[case::min("-8.64e15")]
    fn a_date_keeps_the_time_value(#[case] time: &str) {
        assert_eq!(
            run(&format!(
                "String(Object.is(structuredClone(new Date({time})).getTime(), new Date({time}).getTime()))"
            )),
            "true"
        );
    }

    #[test]
    fn a_date_subclass_gives_a_plain_date() {
        assert_eq!(
            run(r"
                class D extends Date {}
                String(Object.getPrototypeOf(structuredClone(new D(0))) === Date.prototype);
                "),
            "true"
        );
    }

    #[rstest]
    #[case::has_indices("d")]
    #[case::global("g")]
    #[case::ignore_case("i")]
    #[case::multiline("m")]
    #[case::dot_all("s")]
    #[case::unicode("u")]
    #[case::unicode_sets("v")]
    #[case::sticky("y")]
    fn a_regexp_keeps_the_source_and_each_flag(#[case] flag: &str) {
        assert_eq!(
            run(&format!(
                r"
                const c = structuredClone(new RegExp('a.b', '{flag}'));
                `${{c.source}}|${{c.flags}}`;
                "
            )),
            format!("a.b|{flag}")
        );
    }

    #[test]
    fn a_regexp_clone_starts_at_last_index_zero() {
        assert_eq!(
            run(r"
                const source = /a/g;
                source.lastIndex = 3;
                String(structuredClone(source).lastIndex);
                "),
            "0"
        );
    }

    #[test]
    fn an_own_global_property_on_a_regexp_changes_no_flag() {
        assert_eq!(
            run(r"
                const source = /a/;
                Object.defineProperty(source, 'global', { value: true });
                structuredClone(source).flags;
                "),
            ""
        );
    }

    #[test]
    fn a_regexp_subclass_gives_a_plain_regexp() {
        assert_eq!(
            run(r"
                class R extends RegExp {}
                String(Object.getPrototypeOf(structuredClone(new R('a'))) === RegExp.prototype);
                "),
            "true"
        );
    }

    #[test]
    fn a_boxed_number_subclass_gives_a_plain_number_object() {
        assert_eq!(
            run(r"
                class N extends Number {}
                String(Object.getPrototypeOf(structuredClone(new N(1))) === Number.prototype);
                "),
            "true"
        );
    }

    #[rstest]
    #[case::date("new Date(0)")]
    #[case::boxed_number("Object(1)")]
    fn an_own_property_of_a_leaf_is_not_copied(#[case] source: &str) {
        assert_eq!(
            run(&format!(
                r"
                const source = {source};
                source.extra = 1;
                String('extra' in structuredClone(source));
                "
            )),
            "false"
        );
    }

    #[rstest]
    #[case::object("Object", "Object(5)")]
    #[case::date("Date", "new Date(5)")]
    #[case::regexp("RegExp", "/a/")]
    fn a_replaced_global_changes_no_leaf_clone(#[case] global: &str, #[case] source: &str) {
        assert_eq!(
            run(&format!(
                r"
                const source = {source};
                globalThis.{global} = function () {{
                    throw new Error('used');
                }};
                typeof structuredClone(source);
                "
            )),
            "object"
        );
    }

    #[rstest]
    #[case::number("Number", "Object(5)", "5")]
    #[case::string("String", "Object('s')", "s")]
    #[case::boolean("Boolean", "Object(true)", "true")]
    #[case::bigint("BigInt", "Object(5n)", "5")]
    fn a_replaced_value_of_changes_no_boxed_clone(
        #[case] class: &str,
        #[case] source: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            run(&format!(
                r"
                const source = {source};
                const original = {class}.prototype.valueOf;
                {class}.prototype.valueOf = () => 99;
                String(original.call(structuredClone(source)));
                "
            )),
            expected
        );
    }

    #[test]
    fn a_replaced_get_time_changes_no_date_clone() {
        assert_eq!(
            run(r"
                const source = new Date(7);
                const original = Date.prototype.getTime;
                Date.prototype.getTime = () => 99;
                String(original.call(structuredClone(source)));
                "),
            "7"
        );
    }

    #[test]
    fn a_replaced_flags_getter_changes_no_regexp_clone() {
        assert_eq!(
            run(r"
                const source = /a/gi;
                const original = Object.getOwnPropertyDescriptor(RegExp.prototype, 'flags').get;
                Object.defineProperty(RegExp.prototype, 'flags', {
                    get() {
                        return 'x';
                    },
                });
                original.call(structuredClone(source));
                "),
            "gi"
        );
    }

    #[test]
    fn a_replaced_source_getter_changes_no_regexp_clone() {
        assert_eq!(
            run(r"
                const source = /abc/;
                const original = Object.getOwnPropertyDescriptor(RegExp.prototype, 'source').get;
                Object.defineProperty(RegExp.prototype, 'source', {
                    get() {
                        return 'x';
                    },
                });
                original.call(structuredClone(source));
                "),
            "abc"
        );
    }
}
