// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The iterator of a `DOMTokenList`.
//!
//! An iterator holds the node id and a position, and reads the `class` attribute at every `next`
//! call. An iterator of a freed node is done.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

// `TokenListIterator` derives `JsLifetime`, and the class holds a node id and a position and no JS
// value.
#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::atom::PredefinedAtom;
use rquickjs::object::Property;
use rquickjs::{Array, Class, Ctx, IntoJs, Object, Result as JsResult, Value};

use inca_jsenv::class::{define_this_method, iterator_prototype, plain_class, receiver};
use inca_jsenv::store_prototype;
use inca_node::NodeId;

use super::list::tokens_of;
use crate::bindings::convert::saturated;
use crate::bindings::host::Host;
use inca_jsenv::required_prototype;

/// The name of the iterator prototype in the prototype store.
const ITERATOR: &str = "DOMTokenList Iterator";

/// What an iterator yields for a token.
#[derive(Clone, Copy)]
pub(super) enum Kind {
    Keys,
    Values,
    Entries,
}

/// A `DOMTokenList` iterator. It holds the node id and the position, and no JS value.
// `TokenListIterator` holds a node id and a position and no JS value, so the derive is sound.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
struct TokenListIterator {
    node: NodeId,
    kind: Kind,
    index: Cell<usize>,
}

plain_class!(TokenListIterator, "DOMTokenList Iterator");

/// Defines the iterator prototype with `next` and keeps it in the prototype store.
pub(super) fn install_iterator(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
    let iterator = iterator_prototype(ctx)?;
    define_next(ctx, &iterator, host)?;
    store_prototype(ctx, ITERATOR, &iterator)
}

/// A new iterator over the list of `node` that yields what `kind` names.
pub(super) fn new_iterator<'js>(ctx: &Ctx<'js>, node: NodeId, kind: Kind) -> JsResult<Value<'js>> {
    let iterator = TokenListIterator {
        node,
        kind,
        index: Cell::new(0),
    };
    Ok(Class::instance_proto(iterator, required_prototype(ctx, ITERATOR)?)?.into_value())
}

fn define_next<'js>(
    ctx: &Ctx<'js>,
    iterator: &Object<'js>,
    host: &Rc<RefCell<Host>>,
) -> JsResult<()> {
    iterator.prop(
        PredefinedAtom::SymbolToStringTag,
        Property::from("Array Iterator").configurable(),
    )?;
    let h = host.clone();
    define_this_method(ctx, iterator, "next", 0, move |ctx, this, _| {
        let class = receiver::<TokenListIterator>(ctx, &this)?;
        let it = class.borrow();
        let index = it.index.get();
        let token = tokens_of(&h, it.node).into_iter().nth(index);
        let result = Object::new(ctx.clone())?;
        let Some(token) = token else {
            // An exhausted iterator stays exhausted when the list grows.
            it.index.set(usize::MAX);
            result.set("value", Value::new_undefined(ctx.clone()))?;
            result.set("done", true)?;
            return Ok(result);
        };
        it.index.set(index + 1);
        let value = match it.kind {
            Kind::Keys => saturated(index).into_js(ctx)?,
            Kind::Values => token.into_js(ctx)?,
            Kind::Entries => {
                let pair = Array::new(ctx.clone())?;
                pair.set(0, saturated(index))?;
                pair.set(1, token)?;
                pair.into_value()
            }
        };
        result.set("value", value)?;
        result.set("done", false)?;
        Ok(result)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::tests::list_run;

    #[test]
    fn an_iterator_has_the_array_iterator_tag() {
        assert_eq!(
            list_run(
                None,
                "return Object.prototype.toString.call(list.values());"
            ),
            json!("[object Array Iterator]")
        );
    }

    #[test]
    fn an_iterator_returns_itself_as_its_iterator() {
        let body = r"
            const it = list.values();
            return it[Symbol.iterator]() === it;
            ";

        assert_eq!(list_run(Some("a"), body), json!(true));
    }

    #[test]
    fn an_iterator_spreads_into_an_array() {
        assert_eq!(
            list_run(Some("a b"), "return [...list.values()];"),
            json!(["a", "b"])
        );
    }

    #[test]
    fn a_for_of_loop_walks_the_keys() {
        let body = r"
            const seen = [];
            for (const key of list.keys()) {
                seen.push(key);
            }
            return seen;
            ";

        assert_eq!(list_run(Some("a b"), body), json!([0, 1]));
    }

    #[test]
    fn entries_yield_the_index_and_the_token() {
        assert_eq!(
            list_run(Some("a b"), "return [...list.entries()];"),
            json!([[0, "a"], [1, "b"]])
        );
    }

    #[test]
    fn an_iterator_visits_a_token_added_during_the_walk() {
        let body = r"
            const seen = [];
            for (const token of list) {
                list.add('b');
                seen.push(token);
            }
            return seen;
            ";

        assert_eq!(list_run(Some("a"), body), json!(["a", "b"]));
    }

    #[test]
    fn an_iterator_skips_a_token_removed_during_the_walk() {
        let body = r"
            const seen = [];
            for (const token of list) {
                list.remove('b');
                seen.push(token);
            }
            return seen;
            ";

        assert_eq!(list_run(Some("a b c"), body), json!(["a", "c"]));
    }

    #[test]
    fn an_exhausted_iterator_stays_done_when_the_list_grows() {
        let body = r"
            const it = list.values();
            it.next();
            it.next();
            list.add('b');
            return it.next();
            ";

        assert_eq!(list_run(Some("a"), body), json!({ "done": true }));
    }
}
