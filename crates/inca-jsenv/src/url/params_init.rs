// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The conversion of the argument of the `URLSearchParams` constructor.

use std::collections::HashMap;

use rquickjs::atom::PredefinedAtom;
use rquickjs::function::This;
use rquickjs::object::Filter;
use rquickjs::{Atom, Ctx, Error, Exception, Function, Object, Result as JsResult, Value};
use url::form_urlencoded;

use super::usv;

type List = Vec<(String, String)>;

const NOT_ITERABLE: &str =
    "the object is not iterable. pass an iterable of pairs or a plain object";
const ITEM_NOT_ITERABLE: &str = "an item of the sequence is not iterable. pass [name, value] pairs";
const WRONG_PAIR_LENGTH: &str =
    "a pair of the sequence does not have two items. pass [name, value] pairs";
const SYMBOL_KEY: &str = "a key of the record is a symbol. use string keys";

/// Calls `each` with every value `method` yields when called on `iterable`. A throw of `each`
/// closes the iterator and propagates.
fn iterate<'js>(
    ctx: &Ctx<'js>,
    iterable: &Value<'js>,
    method: &Function<'js>,
    mut each: impl FnMut(Value<'js>) -> JsResult<()>,
) -> JsResult<()> {
    let iterator: Object = method.call((This(iterable.clone()),))?;
    let next: Function = iterator.get("next")?;
    loop {
        let result: Object = next.call((This(iterator.clone()),))?;
        if result.get::<_, rquickjs::convert::Coerced<bool>>("done")?.0 {
            return Ok(());
        }
        if let Err(error) = each(result.get("value")?) {
            if !matches!(error, Error::Exception) {
                return Err(error);
            }
            let exception = ctx.catch();
            if let Ok(close) = iterator.get::<_, Function>("return") {
                let _ = close.call::<_, Value>((This(iterator.clone()),));
            }
            drop(ctx.catch());
            return Err(ctx.throw(exception));
        }
    }
}

/// The pairs `init` describes: a sequence of pairs, a record or a query string.
pub(super) fn init_list<'js>(ctx: &Ctx<'js>, init: Option<&Value<'js>>) -> JsResult<List> {
    let Some(init) = init else {
        return Ok(Vec::new());
    };
    let Some(object) = init.as_object() else {
        let text = usv(init)?;
        let query = text.strip_prefix('?').unwrap_or(&text);
        return Ok(form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect());
    };
    let method: Value = object.get(PredefinedAtom::SymbolIterator)?;
    if method.is_undefined() || method.is_null() {
        return record(ctx, object);
    }
    let method = method
        .into_function()
        .ok_or_else(|| Exception::throw_type(ctx, NOT_ITERABLE))?;
    let mut list = Vec::new();
    iterate(ctx, init, &method, |item| {
        list.push(pair_of(ctx, &item)?);
        Ok(())
    })?;
    Ok(list)
}

/// The pair an item of a sequence holds.
fn pair_of<'js>(ctx: &Ctx<'js>, item: &Value<'js>) -> JsResult<(String, String)> {
    let not_iterable = || Exception::throw_type(ctx, ITEM_NOT_ITERABLE);
    let method = item
        .as_object()
        .ok_or_else(not_iterable)?
        .get::<_, Value>(PredefinedAtom::SymbolIterator)?
        .into_function()
        .ok_or_else(not_iterable)?;
    let mut items = Vec::new();
    iterate(ctx, item, &method, |value| {
        items.push(usv(&value)?);
        Ok(())
    })?;
    let [name, value] = <[String; 2]>::try_from(items)
        .map_err(|_| Exception::throw_type(ctx, WRONG_PAIR_LENGTH))?;
    Ok((name, value))
}

/// The pairs of the own enumerable string keys of `object`, in own-keys order. An enumerable
/// symbol key throws. A repeated name keeps its first place and takes the later value.
fn record<'js>(ctx: &Ctx<'js>, object: &Object<'js>) -> JsResult<List> {
    let mut list: List = Vec::new();
    let mut places: HashMap<String, usize> = HashMap::new();
    for key in object.own_keys::<Atom>(Filter::new().string().symbol().enum_only()) {
        let key = key?;
        let text = key.to_value()?;
        // A symbol atom converts to its description, which names a different atom.
        if Atom::from_value(ctx.clone(), &text)? != key {
            return Err(Exception::throw_type(ctx, SYMBOL_KEY));
        }
        let name = usv(&text)?;
        let value = usv(&object.get::<_, Value>(key)?)?;
        if let Some(&place) = places.get(&name) {
            list[place].1 = value;
        } else {
            places.insert(name.clone(), list.len());
            list.push((name, value));
        }
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::{ITEM_NOT_ITERABLE, NOT_ITERABLE, SYMBOL_KEY, WRONG_PAIR_LENGTH};
    use crate::tests::{run, thrown};

    #[rstest]
    #[case::none("new URLSearchParams()")]
    #[case::undefined("new URLSearchParams(undefined)")]
    fn the_constructor_without_an_argument_or_undefined_is_empty(#[case] source: &str) {
        assert_eq!(run(&format!("{source}.toString()")), "");
    }

    #[rstest]
    #[case::one("?a=1", "a=1")]
    #[case::two("??a", "%3Fa=")]
    fn the_constructor_removes_one_leading_question_mark(
        #[case] init: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            run(&format!("new URLSearchParams('{init}').toString()")),
            expected
        );
    }

    #[rstest]
    #[case::array("[['a', '1'], ['b', '2']]")]
    #[case::map("new Map([['a', '1'], ['b', '2']])")]
    #[case::params("new URLSearchParams('a=1&b=2')")]
    #[case::generator(
        r"(function* () {
            yield ['a', '1'];
            yield ['b', '2'];
        })()"
    )]
    fn the_constructor_takes_pairs_from_a_sequence(#[case] init: &str) {
        assert_eq!(
            run(&format!("new URLSearchParams({init}).toString()")),
            "a=1&b=2"
        );
    }

    #[test]
    fn the_constructor_follows_a_patched_iterator_of_a_copied_params() {
        assert_eq!(
            run(r"
                const source = new URLSearchParams('a=1');
                source[Symbol.iterator] = function* () {
                    yield ['patched', '2'];
                };
                new URLSearchParams(source).toString();
                "),
            "patched=2"
        );
    }

    #[rstest]
    #[case::one("[['a']]")]
    #[case::three("[['a', 'b', 'c']]")]
    fn a_sequence_item_with_a_wrong_length_throws(#[case] init: &str) {
        assert_eq!(
            thrown(&format!("new URLSearchParams({init})")),
            format!("TypeError: {WRONG_PAIR_LENGTH}")
        );
    }

    #[rstest]
    #[case::number("[1]")]
    #[case::null("[null]")]
    fn a_sequence_item_that_is_not_iterable_throws(#[case] init: &str) {
        assert_eq!(
            thrown(&format!("new URLSearchParams({init})")),
            format!("TypeError: {ITEM_NOT_ITERABLE}")
        );
    }

    #[test]
    fn a_sequence_error_closes_the_iterator() {
        assert_eq!(
            run(r"
                let closed = false;
                const init = (function* () {
                    try {
                        yield ['a'];
                    } finally {
                        closed = true;
                    }
                })();
                try {
                    new URLSearchParams(init);
                } catch (e) {}
                String(closed);
                "),
            "true"
        );
    }

    #[test]
    fn the_constructor_takes_a_record_in_key_order() {
        assert_eq!(
            run("new URLSearchParams({ b: 1, 2: 'x', a: 3 }).toString()"),
            "2=x&b=1&a=3"
        );
    }

    #[test]
    fn a_record_skips_keys_that_are_not_own_enumerable() {
        assert_eq!(
            run(r"
                const init = Object.create({ inherited: 1 });
                init.shown = 2;
                Object.defineProperty(init, 'hidden', { value: 3 });
                new URLSearchParams(init).toString();
                "),
            "shown=2"
        );
    }

    #[rstest]
    #[case::symbol_key("{ b: 2, [Symbol()]: 1 }")]
    #[case::symbol_iterator_null("{ [Symbol.iterator]: null, a: 1 }")]
    fn a_record_with_an_enumerable_symbol_key_throws(#[case] init: &str) {
        assert_eq!(
            thrown(&format!("new URLSearchParams({init})")),
            format!("TypeError: {SYMBOL_KEY}")
        );
    }

    #[test]
    fn a_proxy_record_takes_its_keys_from_the_traps() {
        assert_eq!(
            run(r"
                const values = { x: '1', y: '2' };
                const traps = {
                    ownKeys: () => ['x', 'y'],
                    getOwnPropertyDescriptor: (target, key) => ({
                        value: values[key],
                        enumerable: true,
                        configurable: true,
                    }),
                    get: (target, key) => values[key],
                };
                new URLSearchParams(new Proxy({}, traps)).toString();
                "),
            "x=1&y=2"
        );
    }

    #[test]
    fn a_record_with_a_throwing_getter_throws_the_error() {
        assert_eq!(
            thrown(
                r"new URLSearchParams({
                    get a() {
                        throw new RangeError('boom');
                    },
                })"
            ),
            "RangeError: boom"
        );
    }

    #[test]
    fn an_object_with_a_non_enumerable_null_iterator_is_a_record() {
        assert_eq!(
            run(r"
                const init = { a: 1 };
                Object.defineProperty(init, Symbol.iterator, { value: null });
                new URLSearchParams(init).toString();
                "),
            "a=1"
        );
    }

    #[test]
    fn a_non_callable_iterator_throws() {
        assert_eq!(
            thrown("new URLSearchParams({ [Symbol.iterator]: 1 })"),
            format!("TypeError: {NOT_ITERABLE}")
        );
    }

    #[rstest]
    #[case::null("null", "null=")]
    #[case::number("5", "5=")]
    #[case::boolean("true", "true=")]
    fn a_non_object_init_is_converted_to_a_string(#[case] init: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!("new URLSearchParams({init}).toString()")),
            expected
        );
    }

    #[test]
    fn a_lone_surrogate_in_a_pair_becomes_u_fffd() {
        assert_eq!(
            run(r"new URLSearchParams([['\uD800', '\uDC00x']]).toString()"),
            "%EF%BF%BD=%EF%BF%BDx"
        );
    }

    #[rstest]
    #[case::adjacent(r"{ '\uD800': '1', '\uD801': '2' }", "%EF%BF%BD=2")]
    #[case::separated(r"{ '\uD800': '1', b: '2', '\uD801': '3' }", "%EF%BF%BD=3&b=2")]
    fn a_repeated_record_name_after_conversion_takes_the_later_value(
        #[case] init: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            run(&format!("new URLSearchParams({init}).toString()")),
            expected
        );
    }

    #[test]
    fn a_copy_of_search_params_is_independent() {
        assert_eq!(
            run(r"
                const source = new URLSearchParams('a=1');
                const copy = new URLSearchParams(source);
                copy.append('b', '2');
                source.toString() + '|' + copy.toString();
                "),
            "a=1|a=1&b=2"
        );
    }
}
