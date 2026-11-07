// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `URLSearchParams Iterator` class.

use std::cell::Cell;

#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::atom::PredefinedAtom;
use rquickjs::function::This;
use rquickjs::object::Property;
use rquickjs::{Ctx, Error, IntoJs, Object, Result as JsResult, Value};

use super::{Shared, method, receiver};
use crate::class::plain_class;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Entries,
    Keys,
    Values,
}

/// A `URLSearchParams Iterator` instance. It holds no JS value.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(super) struct ParamsIterator {
    pub(super) state: Shared,
    pub(super) kind: Kind,
    pub(super) index: Cell<usize>,
}

plain_class!(ParamsIterator, "URLSearchParams Iterator");

pub(super) fn define_iterator<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    proto.prop(
        PredefinedAtom::SymbolToStringTag,
        Property::from("URLSearchParams Iterator").configurable(),
    )?;
    method(
        ctx,
        proto,
        "next",
        0,
        |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
            let class = receiver::<ParamsIterator>(&ctx, &this)?;
            let pair = {
                let it = class.borrow();
                let index = it.index.get();
                let pair = it.state.borrow().pair_at(index);
                if pair.is_some() {
                    it.index.set(index + 1);
                }
                pair.map(|pair| (it.kind, pair))
            };
            let (value, done) = match pair {
                Some((Kind::Entries, (name, value))) => (vec![name, value].into_js(&ctx)?, false),
                Some((Kind::Keys, (name, _))) => (name.into_js(&ctx)?, false),
                Some((Kind::Values, (_, value))) => (value.into_js(&ctx)?, false),
                None => (Value::new_undefined(ctx.clone()), true),
            };
            let result = Object::new(ctx)?;
            result.set("value", value)?;
            result.set("done", done)?;
            Ok::<_, Error>(result)
        },
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::tests::{run, thrown};

    #[rstest]
    #[case::entries("entries()", "a,1;b,2")]
    #[case::keys("keys()", "a;b")]
    #[case::values("values()", "1;2")]
    fn each_iterator_kind_yields_its_part_of_the_pair(#[case] call: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!(
                "[...new URLSearchParams('a=1&b=2').{call}].join(';')"
            )),
            expected
        );
    }

    #[test]
    fn the_default_iterator_is_entries() {
        assert_eq!(
            run(
                "String(URLSearchParams.prototype[Symbol.iterator] === URLSearchParams.prototype.entries)"
            ),
            "true"
        );
    }

    #[test]
    fn the_iterator_inherits_from_the_iterator_prototype() {
        assert_eq!(
            run(r"
                const it = new URLSearchParams().keys();
                const base = Object.getPrototypeOf(Object.getPrototypeOf([][Symbol.iterator]()));
                String(base.isPrototypeOf(it));
                "),
            "true"
        );
    }

    #[test]
    fn the_iterator_returns_itself_as_its_iterator() {
        assert_eq!(
            run(r"
                const it = new URLSearchParams().keys();
                String(it[Symbol.iterator]() === it);
                "),
            "true"
        );
    }

    #[test]
    fn next_returns_a_plain_result_object() {
        assert_eq!(
            run(r"
                const it = new URLSearchParams('a=1').keys();
                const first = it.next();
                const last = it.next();
                [Object.keys(first), first.value, first.done, last.value, last.done].join('|');
                "),
            "value,done|a|false||true"
        );
    }

    #[test]
    fn an_exhausted_iterator_resumes_after_an_append() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1');
                const it = p.keys();
                it.next();
                it.next();
                p.append('b', '2');
                it.next().value;
                "),
            "b"
        );
    }

    #[test]
    fn a_deletion_during_iteration_skips_the_next_pair() {
        assert_eq!(
            run(r"
                const p = new URLSearchParams('a=1&b=2&c=3');
                const seen = [];
                for (const name of p.keys()) {
                    seen.push(name);
                    if (name === 'a') {
                        p.delete('a');
                    }
                }
                seen.join();
                "),
            "a,c"
        );
    }

    #[test]
    fn next_on_a_wrong_receiver_throws() {
        assert_eq!(
            thrown("new URLSearchParams().keys().next.call({})"),
            "TypeError: the receiver is not a URLSearchParams Iterator. call the method on an instance"
        );
    }
}
