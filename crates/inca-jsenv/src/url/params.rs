// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `URLSearchParams` class.

use rquickjs::atom::PredefinedAtom;
use rquickjs::function::{IntoJsFunc, ParamRequirement, Params, Rest, This};
use rquickjs::object::Property;
use rquickjs::{Class, Ctx, Error, Exception, Function, IntoJs, Object, Result as JsResult, Value};

use super::iterator::{Kind, ParamsIterator};
use super::params_init::init_list;
use super::{ParamsInstance, STORE, State, given, method, require, shared, usv};
use crate::class::{NEEDS_NEW, define_class, define_getter, define_global, new_target_prototype};
use crate::vault::vault;

const NOT_A_FUNCTION: &str = "the callback is not a function. pass a function";

/// The body of the constructor.
struct Construct;

impl<'js> IntoJsFunc<'js, ()> for Construct {
    fn param_requirements() -> ParamRequirement {
        ParamRequirement::any()
    }

    fn call<'a>(&self, params: Params<'a, 'js>) -> JsResult<Value<'js>> {
        let ctx = params.ctx();
        if !params.is_constructor() {
            return Err(Exception::throw_type(ctx, NEEDS_NEW));
        }
        let init = params.arg(0).filter(|value| !value.is_undefined());
        let list = init_list(ctx, init.as_ref())?;
        let store: Object = vault(ctx)?.get(STORE)?;
        let proto = new_target_prototype(&params, store.get("params")?)?;
        let instance = ParamsInstance(State::new(None, list));
        Ok(Class::instance_proto(instance, proto)?.into_value())
    }
}

/// The name and the optional value of `delete` and `has`.
fn name_and_value(
    ctx: &Ctx<'_>,
    method: &str,
    args: &[Value<'_>],
) -> JsResult<(String, Option<String>)> {
    require(ctx, method, args, 1)?;
    let name = usv(&args[0])?;
    let value = given(args, 1).map(usv).transpose()?;
    Ok((name, value))
}

/// The name and the value of `append` and `set`.
fn name_value(ctx: &Ctx<'_>, method: &str, args: &[Value<'_>]) -> JsResult<(String, String)> {
    require(ctx, method, args, 2)?;
    Ok((usv(&args[0])?, usv(&args[1])?))
}

/// The name of `get` and `getAll`.
fn name_of(ctx: &Ctx<'_>, method: &str, args: &[Value<'_>]) -> JsResult<String> {
    require(ctx, method, args, 1)?;
    usv(&args[0])
}

/// A new iterator of `kind` over the pairs of `this`.
fn iterator<'js>(ctx: &Ctx<'js>, this: &Value<'js>, kind: Kind) -> JsResult<Value<'js>> {
    let state = shared(ctx, this)?;
    let store: Object = vault(ctx)?.get(STORE)?;
    let iterator = ParamsIterator {
        state,
        kind,
        index: std::cell::Cell::new(0),
    };
    Ok(Class::instance_proto(iterator, store.get("iterator")?)?.into_value())
}

/// Defines `append`, `delete`, `set` and `sort`, which change the list.
fn define_changing_members<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    method(
        ctx,
        proto,
        "append",
        2,
        |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            let state = shared(&ctx, &this)?;
            let (name, value) = name_value(&ctx, "append", &args)?;
            let mut state = state.borrow_mut();
            state.list.push((name, value));
            state.write();
            Ok::<_, Error>(())
        },
    )?;
    method(
        ctx,
        proto,
        "delete",
        1,
        |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            let state = shared(&ctx, &this)?;
            let (name, value) = name_and_value(&ctx, "delete", &args)?;
            let mut state = state.borrow_mut();
            state
                .list
                .retain(|(n, v)| !(*n == name && value.as_ref().is_none_or(|value| value == v)));
            state.write();
            Ok::<_, Error>(())
        },
    )?;
    method(
        ctx,
        proto,
        "set",
        2,
        |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            let state = shared(&ctx, &this)?;
            let (name, value) = name_value(&ctx, "set", &args)?;
            let mut state = state.borrow_mut();
            let mut found = false;
            state.list.retain_mut(|pair| {
                if pair.0 != name {
                    return true;
                }
                if found {
                    return false;
                }
                found = true;
                pair.1.clone_from(&value);
                true
            });
            if !found {
                state.list.push((name, value));
            }
            state.write();
            Ok::<_, Error>(())
        },
    )?;
    method(
        ctx,
        proto,
        "sort",
        0,
        |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
            let state = shared(&ctx, &this)?;
            let mut state = state.borrow_mut();
            state
                .list
                .sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            state.write();
            Ok::<_, Error>(())
        },
    )?;
    Ok(())
}

/// Defines `get`, `getAll`, `has` and `toString`, which read the list.
fn define_reading_members<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    method(
        ctx,
        proto,
        "get",
        1,
        |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            let state = shared(&ctx, &this)?;
            let name = name_of(&ctx, "get", &args)?;
            let found = state
                .borrow()
                .list
                .iter()
                .find(|pair| pair.0 == name)
                .map(|pair| pair.1.clone());
            match found {
                Some(value) => value.into_js(&ctx),
                None => Ok(Value::new_null(ctx)),
            }
        },
    )?;
    method(
        ctx,
        proto,
        "getAll",
        1,
        |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            let state = shared(&ctx, &this)?;
            let name = name_of(&ctx, "getAll", &args)?;
            let found: Vec<String> = state
                .borrow()
                .list
                .iter()
                .filter(|pair| pair.0 == name)
                .map(|pair| pair.1.clone())
                .collect();
            found.into_js(&ctx)
        },
    )?;
    method(
        ctx,
        proto,
        "has",
        1,
        |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            let state = shared(&ctx, &this)?;
            let (name, value) = name_and_value(&ctx, "has", &args)?;
            let found = state
                .borrow()
                .list
                .iter()
                .any(|(n, v)| *n == name && value.as_ref().is_none_or(|value| value == v));
            Ok::<_, Error>(found)
        },
    )?;
    method(
        ctx,
        proto,
        "toString",
        0,
        |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
            let state = shared(&ctx, &this)?;
            let text = state.borrow().serialize();
            Ok::<_, Error>(text)
        },
    )?;
    Ok(())
}

pub(super) fn define_params<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    let class = define_class(ctx, proto, "URLSearchParams", 0, Construct)?;
    define_changing_members(ctx, proto)?;
    define_reading_members(ctx, proto)?;
    method(
        ctx,
        proto,
        "forEach",
        1,
        |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            let state = shared(&ctx, &this)?;
            require(&ctx, "forEach", &args, 1)?;
            let callback = args[0]
                .as_function()
                .cloned()
                .ok_or_else(|| Exception::throw_type(&ctx, NOT_A_FUNCTION))?;
            let this_arg = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| Value::new_undefined(ctx.clone()));
            let mut index = 0;
            loop {
                let pair = state.borrow().pair_at(index);
                let Some((name, value)) = pair else {
                    return Ok::<_, Error>(());
                };
                callback.call::<_, ()>((This(this_arg.clone()), value, name, this.clone()))?;
                index += 1;
            }
        },
    )?;
    define_getter(
        proto,
        "size",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
                let state = shared(&ctx, &this)?;
                let size = state.borrow().list.len();
                Ok::<_, Error>(size)
            },
        )?,
    )?;
    for (name, kind) in [
        ("entries", Kind::Entries),
        ("keys", Kind::Keys),
        ("values", Kind::Values),
    ] {
        method(
            ctx,
            proto,
            name,
            0,
            move |ctx: Ctx<'js>, This(this): This<Value<'js>>| iterator(&ctx, &this, kind),
        )?;
    }
    let entries: Function = proto.get("entries")?;
    proto.prop(
        PredefinedAtom::SymbolIterator,
        Property::from(entries).writable().configurable(),
    )?;
    define_global(ctx, "URLSearchParams", class)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::NOT_A_FUNCTION;
    use crate::Engine;
    use crate::tests::{around, run, thrown};

    #[test]
    fn append_adds_a_duplicate_name() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1');
                p.append('a', '2');
                p.toString();
                "),
            "a=1&a=2"
        );
    }

    #[test]
    fn delete_removes_every_pair_with_the_name() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1&b=2&a=3');
                p.delete('a');
                p.toString();
                "),
            "b=2"
        );
    }

    #[test]
    fn delete_with_a_value_removes_only_matching_pairs() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1&a=2&a=1');
                p.delete('a', '1');
                p.toString();
                "),
            "a=2"
        );
    }

    #[rstest]
    #[case::delete(
        r"
        const p = new URLSearchParams('a=1&a=2');
        p.delete('a', undefined);
        String(p.size);
        ",
        "0"
    )]
    #[case::has("String(new URLSearchParams('a=1').has('a', undefined))", "true")]
    fn an_undefined_value_argument_is_absent(#[case] source: &str, #[case] expected: &str) {
        assert_eq!(run(source), expected);
    }

    #[test]
    fn delete_treats_a_null_value_as_the_string_null() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1');
                p.delete('a', null);
                String(p.size);
                "),
            "1"
        );
    }

    #[rstest]
    #[case::matching("a", "2", "true")]
    #[case::other_value("a", "3", "false")]
    fn has_with_a_value_checks_the_pair(
        #[case] name: &str,
        #[case] value: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            run(&format!(
                "String(new URLSearchParams('a=1&a=2').has('{name}', '{value}'))"
            )),
            expected
        );
    }

    #[rstest]
    #[case::first("a", "1")]
    #[case::absent("z", "null")]
    fn get_returns_the_first_value_or_null(#[case] name: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!(
                "String(new URLSearchParams('a=1&a=2').get('{name}'))"
            )),
            expected
        );
    }

    #[test]
    fn get_all_returns_every_value_in_order() {
        assert_eq!(
            run("new URLSearchParams('a=1&b=0&a=2').getAll('a').join()"),
            "1,2"
        );
    }

    #[test]
    fn set_replaces_the_first_pair_in_place() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1&b=2&a=3');
                p.set('a', '9');
                p.getAll('a').join() + '|' + [...p.keys()].join();
                "),
            "9|a,b"
        );
    }

    #[test]
    fn set_removes_the_other_pairs_with_the_name() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1&b=2&a=3');
                p.set('a', '9');
                String(p.size);
                "),
            "2"
        );
    }

    #[test]
    fn set_appends_when_the_name_is_absent() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1');
                p.set('b', '2');
                p.toString();
                "),
            "a=1&b=2"
        );
    }

    #[test]
    fn size_counts_the_pairs() {
        assert_eq!(run("String(new URLSearchParams('a=1&b=2').size)"), "2");
    }

    #[test]
    fn size_is_an_enumerable_configurable_getter_without_setter() {
        assert_eq!(
            run(r"
                const proto = URLSearchParams.prototype;
                const d = Object.getOwnPropertyDescriptor(proto, 'size');
                [d.enumerable, d.configurable, !!d.get, !!d.set].join();
                "),
            "true,true,true,false"
        );
    }

    #[rstest]
    #[case::space("' '", "+")]
    #[case::tilde("'~'", "%7E")]
    #[case::quote("\"'\"", "%27")]
    #[case::accent("'é'", "%C3%A9")]
    #[case::astral("'😀'", "%F0%9F%98%80")]
    #[case::ampersand("'&'", "%26")]
    #[case::equals("'='", "%3D")]
    #[case::newline("'\\n'", "%0A")]
    fn to_string_serializes_each_character(#[case] value: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!("new URLSearchParams([['k', {value}]]).toString()")),
            format!("k={expected}")
        );
    }

    #[rstest]
    #[case::append("append('a')", "append", 2)]
    #[case::set("set('a')", "set", 2)]
    #[case::get("get()", "get", 1)]
    #[case::get_all("getAll()", "getAll", 1)]
    #[case::has("has()", "has", 1)]
    #[case::delete("delete()", "delete", 1)]
    #[case::for_each("forEach()", "forEach", 1)]
    fn a_member_with_too_few_arguments_throws(
        #[case] call: &str,
        #[case] method: &str,
        #[case] count: usize,
    ) {
        assert_eq!(
            thrown(&format!("new URLSearchParams().{call}")),
            format!("TypeError: too few arguments for {method}. pass at least {count}")
        );
    }

    #[test]
    fn a_member_converts_its_arguments_to_strings() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams();
                p.append(1, true);
                p.toString();
                "),
            "1=true"
        );
    }

    #[rstest]
    #[case::append("append", "2")]
    #[case::delete("delete", "1")]
    #[case::get("get", "1")]
    #[case::get_all("getAll", "1")]
    #[case::has("has", "1")]
    #[case::set("set", "2")]
    #[case::sort("sort", "0")]
    #[case::for_each("forEach", "1")]
    #[case::entries("entries", "0")]
    fn a_member_has_its_length(#[case] method: &str, #[case] length: &str) {
        assert_eq!(
            run(&format!(
                "String(URLSearchParams.prototype.{method}.length)"
            )),
            length
        );
    }

    #[test]
    fn sort_orders_names_by_utf_16_code_units() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams();
                for (const name of ['\uFF5E', '😀', '\uE000', '\uFFFF', 'a', 'B', 'é']) {
                    p.append(name, '');
                }
                p.sort();
                [...p.keys()].map((name) => name.codePointAt(0).toString(16)).join();
                "),
            "42,61,e9,1f600,e000,ff5e,ffff"
        );
    }

    #[test]
    fn sort_keeps_the_order_of_equal_names() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams();
                for (let i = 0; i < 40; i++) {
                    p.append('n' + (i % 4), String(i));
                }
                p.sort();
                p.getAll('n1').join();
                "),
            "1,5,9,13,17,21,25,29,33,37"
        );
    }

    #[test]
    fn sort_leaves_values_untouched() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('b=2&a=9&a=1');
                p.sort();
                p.toString();
                "),
            "a=9&a=1&b=2"
        );
    }

    #[test]
    fn for_each_passes_the_value_the_name_and_the_params() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1');
                let seen;
                p.forEach((value, name, object) => {
                    seen = [value, name, object === p];
                });
                seen.join();
                "),
            "1,a,true"
        );
    }

    #[test]
    fn for_each_calls_the_callback_with_the_this_arg() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1');
                const marker = {};
                let seen;
                p.forEach(function () {
                    seen = this === marker;
                }, marker);
                String(seen);
                "),
            "true"
        );
    }

    #[rstest]
    #[case::append("URLSearchParams.prototype.append.call({}, 'a', 'b')")]
    #[case::to_string("URLSearchParams.prototype.toString.call({})")]
    #[case::entries("URLSearchParams.prototype.entries.call({})")]
    #[case::size("Object.getOwnPropertyDescriptor(URLSearchParams.prototype, 'size').get.call({})")]
    fn a_member_on_a_wrong_receiver_throws(#[case] call: &str) {
        assert_eq!(
            thrown(call),
            "TypeError: the receiver is not a URLSearchParams. call the method on an instance"
        );
    }

    #[test]
    fn for_each_visits_a_pair_appended_by_the_callback() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1');
                const seen = [];
                p.forEach((value, name) => {
                    seen.push(name);
                    if (seen.length < 5) {
                        p.append('n' + seen.length, '');
                    }
                });
                String(seen.length);
                "),
            "5"
        );
    }

    #[test]
    fn for_each_with_a_non_function_throws() {
        assert_eq!(
            thrown("new URLSearchParams('a=1').forEach(1)"),
            format!("TypeError: {NOT_A_FUNCTION}")
        );
    }

    #[test]
    fn for_each_mutating_the_same_params_does_not_panic() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1&b=2');
                p.forEach((value, name) => {
                    p.delete(name);
                });
                p.toString();
                "),
            "b=2"
        );
    }

    #[test]
    fn creating_and_dropping_params_leaves_the_counters_at_the_baseline() {
        let engine = Engine::new().unwrap();
        let (before, after) = around(&engine, |ctx| {
            ctx.eval::<(), _>(
                r"
                {
                    const p = new URLSearchParams([['a', '1']]);
                    p.forEach(() => {});
                    [...p];
                }
                ",
            )
            .unwrap();
        });

        assert_eq!(before, after);
    }
}
