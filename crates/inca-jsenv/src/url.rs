// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `URL` and `URLSearchParams` classes.
//!
//! A URL and its `searchParams` object share one `State`: the parsed URL and the list of pairs of
//! its query. Reads use the list. The `search` and `href` setters rebuild the list, and every
//! mutation of the list writes the serialization back to the URL. The prototypes sit in the vault,
//! so a script that replaces a property of a prototype or a constructor changes nothing here.
//! This module installs both classes and holds the state and the shared conversions. The
//! `url_class` module holds `URL`, the `params` module holds `URLSearchParams`, the
//! `params_init` module converts the argument of its constructor and the `iterator` module holds
//! its iterator.

use std::cell::RefCell;
use std::rc::Rc;

#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::function::IntoJsFunc;
use rquickjs::{Class, Ctx, Function, Object, Result as JsResult, Value};
use url::{Url, form_urlencoded, quirks};

use crate::class::{define_method, given, plain_class, receiver, require};
use crate::encoding::encode;
use crate::quickjs::string_wtf8;
use crate::vault::vault;

mod iterator;
mod params;
mod params_init;
mod url_class;

/// The vault key of the prototypes.
const STORE: &str = "url";

/// Evaluates to the prototype of the iterators, derived from the iterator prototype that
/// `Array.prototype[Symbol.iterator]` results inherit from.
const ITERATOR_JS: &str = r"
    (() => {
        const iterators = Object.getPrototypeOf(Object.getPrototypeOf([][Symbol.iterator]()));
        return { __proto__: null, iterator: Object.create(iterators) };
    })()
    ";

/// The URL of a `URL` and the pairs of its query. A `URLSearchParams` of its own has no URL.
struct State {
    url: Option<Url>,
    list: Vec<(String, String)>,
}

type Shared = Rc<RefCell<State>>;

fn serialize(list: &[(String, String)]) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(list)
        .finish()
}

impl State {
    fn new(url: Option<Url>, list: Vec<(String, String)>) -> Shared {
        Rc::new(RefCell::new(Self { url, list }))
    }

    /// Replaces the list with the pairs of the query of the URL.
    fn rebuild(&mut self) {
        let query = self.url.as_ref().and_then(Url::query).unwrap_or_default();
        self.list = form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();
    }

    /// Writes the serialization of the list into the query of the URL, which an empty list removes.
    fn write(&mut self) {
        let Some(url) = &mut self.url else {
            return;
        };
        let query = (!self.list.is_empty()).then(|| serialize(&self.list));
        url.set_query(query.as_deref());
    }

    fn serialize(&self) -> String {
        serialize(&self.list)
    }

    /// Replaces the URL and the list. The URL and the list stay when `href` does not parse.
    fn set_href(&mut self, href: &str) -> Result<(), ()> {
        if let Some(url) = &mut self.url {
            quirks::set_href(url, href).map_err(drop)?;
        }
        self.rebuild();
        Ok(())
    }

    /// Sets the query to `search`, then rebuilds the list from the query the URL stored.
    fn set_search(&mut self, search: &str) {
        let search = if search == "?" { "" } else { search };
        if let Some(url) = &mut self.url {
            quirks::set_search(url, search);
        }
        self.rebuild();
    }

    fn pair_at(&self, index: usize) -> Option<(String, String)> {
        self.list.get(index).cloned()
    }
}

// A derive on a struct with a lifetime generates items that an `allow` on the struct does not
// cover, so the struct sits in a module of its own.
#[allow(clippy::disallowed_macros, clippy::disallowed_types)]
mod instance {
    use rquickjs::class::{JsClass, Readable, Trace, Tracer};
    use rquickjs::function::Constructor;
    use rquickjs::{Class, Ctx, JsLifetime, Object, Result as JsResult};

    use super::{ParamsInstance, Shared};

    /// A `URL` instance. Its `searchParams` object is created with it and traced, so a cycle
    /// through it is collected.
    #[derive(JsLifetime)]
    pub(super) struct UrlInstance<'js> {
        pub(super) state: Shared,
        pub(super) params: Class<'js, ParamsInstance>,
    }

    impl<'js> Trace<'js> for UrlInstance<'js> {
        // Runs inside a GC callback of QuickJS: it must not panic.
        fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
            // No `..` here: a new field must fail to compile until it is traced.
            let UrlInstance { state: _, params } = self;
            params.trace(tracer);
        }
    }

    impl<'js> JsClass<'js> for UrlInstance<'js> {
        const NAME: &'static str = "URL";

        type Mutable = Readable;

        // An instance takes the prototype from the vault when it is made.
        fn prototype(_ctx: &Ctx<'js>) -> JsResult<Option<Object<'js>>> {
            Ok(None)
        }

        fn constructor(_ctx: &Ctx<'js>) -> JsResult<Option<Constructor<'js>>> {
            Ok(None)
        }
    }
}

use instance::UrlInstance;

/// A `URLSearchParams` instance.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
struct ParamsInstance(Shared);

plain_class!(ParamsInstance, "URLSearchParams");

/// Defines the globals `URL` and `URLSearchParams` and keeps their prototypes in the vault of
/// `ctx`'s realm.
///
/// # Errors
///
/// Returns an error if the realm lacks an intrinsic or has no vault.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    let store: Object = ctx.eval(ITERATOR_JS)?;
    let url_proto = Object::new(ctx.clone())?;
    let params_proto = Object::new(ctx.clone())?;
    store.set("url", url_proto.clone())?;
    store.set("params", params_proto.clone())?;
    let iterator_proto: Object = store.get("iterator")?;
    vault(ctx)?.set(STORE, store)?;
    url_class::define_url(ctx, &url_proto)?;
    params::define_params(ctx, &params_proto)?;
    iterator::define_iterator(ctx, &iterator_proto)
}

/// What `console` prints of a `URL` or a `URLSearchParams`, read without running script code.
pub(crate) enum Printed<'js> {
    /// The string members of a `URL` in print order with the `searchParams` object.
    Url {
        members: Vec<(&'static str, String)>,
        params: Value<'js>,
    },
    /// The pairs of a `URLSearchParams` in list order.
    Params(Vec<(String, String)>),
}

/// The state of `value` when it is a `URL` or a `URLSearchParams`, `None` for any other value.
pub(crate) fn printed<'js>(value: &Value<'js>) -> Option<Printed<'js>> {
    if let Ok(class) = Class::<UrlInstance>::from_value(value) {
        let class = class.borrow();
        let state = class.state.borrow();
        let url = state.url.as_ref()?;
        return Some(Printed::Url {
            members: url_class::MEMBERS
                .iter()
                .map(|&(name, read)| (name, read(url)))
                .collect(),
            params: class.params.clone().into_value(),
        });
    }
    let class = Class::<ParamsInstance>::from_value(value).ok()?;
    let pairs = class.borrow().0.borrow().list.clone();
    Some(Printed::Params(pairs))
}

/// `value` converted with `ToString`, with each unpaired surrogate replaced by U+FFFD.
fn usv(value: &Value<'_>) -> JsResult<String> {
    let bytes = encode(&string_wtf8(value)?);
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Defines `body` as the method `name` of `proto` with `length` parameters.
fn method<'js, P>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &str,
    length: usize,
    body: impl IntoJsFunc<'js, P> + 'js,
) -> JsResult<()> {
    define_method(
        proto,
        name,
        Function::new(ctx.clone(), body)?.with_length(length)?,
    )
}

/// The state of the `URLSearchParams` that `this` is.
fn shared<'js>(ctx: &Ctx<'js>, this: &Value<'js>) -> JsResult<Shared> {
    let class = receiver::<ParamsInstance>(ctx, this)?;
    let state = Shared::clone(&class.borrow().0);
    Ok(state)
}

/// The state of the `URL` that `this` is.
fn url_state<'js>(ctx: &Ctx<'js>, this: &Value<'js>) -> JsResult<Shared> {
    let class = receiver::<UrlInstance>(ctx, this)?;
    let state = Rc::clone(&class.borrow().state);
    Ok(state)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::class::NEEDS_NEW;
    use crate::tests::{run, thrown};

    #[rstest]
    #[case::url("URL")]
    #[case::params("URLSearchParams")]
    fn the_global_is_a_function(#[case] name: &str) {
        assert_eq!(run(&format!("typeof {name}")), "function");
    }

    #[rstest]
    #[case::url("URL")]
    #[case::params("URLSearchParams")]
    fn the_global_has_the_attributes_of_a_class_global(#[case] name: &str) {
        let flags = run(&format!(
            r"
            const d = Object.getOwnPropertyDescriptor(globalThis, '{name}');
            [d.writable, d.enumerable, d.configurable].join();
            "
        ));

        assert_eq!(flags, "true,false,true");
    }

    #[rstest]
    #[case::url("URL", "1")]
    #[case::params("URLSearchParams", "0")]
    fn the_constructor_has_its_length(#[case] name: &str, #[case] length: &str) {
        assert_eq!(run(&format!("String({name}.length)")), length);
    }

    #[rstest]
    #[case::url("new URL('http://a/')", "URL")]
    #[case::params("new URLSearchParams()", "URLSearchParams")]
    #[case::iterator("new URLSearchParams().keys()", "URLSearchParams Iterator")]
    fn an_instance_has_the_string_tag_of_its_class(#[case] expression: &str, #[case] tag: &str) {
        assert_eq!(
            run(&format!("Object.prototype.toString.call({expression})")),
            format!("[object {tag}]")
        );
    }

    #[rstest]
    #[case::url("URL('http://a/')")]
    #[case::params("URLSearchParams()")]
    fn calling_the_constructor_without_new_throws(#[case] call: &str) {
        assert_eq!(thrown(call), format!("TypeError: {NEEDS_NEW}"));
    }

    #[rstest]
    #[case::url("URL", "'http://a/'")]
    #[case::params("URLSearchParams", "")]
    fn a_subclass_keeps_its_own_prototype(#[case] base: &str, #[case] argument: &str) {
        assert_eq!(
            run(&format!(
                r"
                class A extends {base} {{
                    hello() {{
                        return 'hi';
                    }}
                }}
                const a = new A({argument});
                [a instanceof A, a instanceof {base}, a.hello()].join();
                "
            )),
            "true,true,hi"
        );
    }

    #[rstest]
    #[case::url("URL", "'http://a/'")]
    #[case::params("URLSearchParams", "")]
    fn a_target_without_a_prototype_object_gives_the_class_prototype(
        #[case] class: &str,
        #[case] argument: &str,
    ) {
        assert_eq!(
            run(&format!(
                r"
                const target = function () {{}};
                target.prototype = null;
                const r = Reflect.construct({class}, [{argument}], target);
                String(Object.getPrototypeOf(r) === {class}.prototype);
                "
            )),
            "true"
        );
    }

    #[test]
    fn search_params_is_the_same_object_on_every_read() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/?x=1');
                const first = u.searchParams;
                u.search = '?y=2';
                const second = u.searchParams;
                u.href = 'http://b/';
                [first === second, second === u.searchParams].join();
                "),
            "true,true"
        );
    }

    #[test]
    fn search_params_starts_with_the_pairs_of_the_query() {
        assert_eq!(
            run("new URL('http://a/?x=1&y=2+3').searchParams.toString()"),
            "x=1&y=2+3"
        );
    }

    #[test]
    fn a_change_of_the_list_writes_the_query() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/p');
                u.searchParams.append('y', '2 3');
                u.href;
                "),
            "http://a/p?y=2+3"
        );
    }

    #[test]
    fn a_change_of_the_list_keeps_the_fragment() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/p?x=1#f');
                u.searchParams.append('y', '2');
                u.hash;
                "),
            "#f"
        );
    }

    #[test]
    fn deleting_the_last_pair_removes_the_question_mark_from_the_href() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/?x=1#f');
                u.searchParams.delete('x');
                u.href;
                "),
            "http://a/#f"
        );
    }

    #[test]
    fn setting_search_after_a_mutation_rebuilds_the_list() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/?x=1');
                u.searchParams.append('x', '1');
                u.search = '?a=2';
                u.searchParams.toString() + '|' + u.searchParams.size;
                "),
            "a=2|1"
        );
    }

    #[test]
    fn setting_search_to_an_empty_string_empties_the_list() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/?x=1');
                u.search = '';
                String(u.searchParams.size);
                "),
            "0"
        );
    }

    #[test]
    fn a_search_of_a_lone_question_mark_removes_the_query() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/?x#y');
                u.search = '?';
                const href = u.href;
                const size = u.searchParams.size;
                u.searchParams.append('a', '1');
                [href, size, u.href].join();
                "),
            "http://a/#y,0,http://a/?a=1#y"
        );
    }

    #[test]
    fn a_search_with_a_hash_encodes_the_hash() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/');
                u.search = '?a#b';
                u.search + '|' + u.searchParams.get('a#b');
                "),
            "?a%23b|"
        );
    }

    #[test]
    fn a_search_with_a_hash_keeps_the_fragment() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/#f');
                u.search = '?a#b';
                u.hash;
                "),
            "#f"
        );
    }

    #[test]
    fn setting_search_inside_for_each_ends_the_walk_on_the_new_list() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/?x=1&y=2&z=3');
                const seen = [];
                u.searchParams.forEach((value, name) => {
                    seen.push(name);
                    u.search = '?a=1&b=2&c=3';
                });
                seen.join();
                "),
            "x,b,c"
        );
    }

    #[test]
    fn an_append_whose_conversion_sets_href_pushes_to_the_rebuilt_list() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/');
                u.searchParams.append('k', {
                    toString() {
                        u.href = 'http://b/?z=1';
                        return 'v';
                    },
                });
                u.href;
                "),
            "http://b/?z=1&k=v"
        );
    }

    #[test]
    fn setting_href_after_a_mutation_rebuilds_the_list() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/?x=1');
                u.searchParams.append('y', '2');
                u.href = 'http://b/?z=3';
                const first = u.searchParams.toString();
                u.href = 'http://c/';
                first + '|' + u.searchParams.size;
                "),
            "z=3|0"
        );
    }

    #[test]
    fn a_failed_href_set_keeps_the_list() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/?x=1');
                try {
                    u.href = 'nope?y=2';
                } catch (e) {}
                u.searchParams.toString();
                "),
            "x=1"
        );
    }

    #[test]
    fn a_mutation_after_a_href_set_writes_into_the_new_url() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/');
                const params = u.searchParams;
                u.href = 'http://b/p';
                params.append('x', '1');
                u.href;
                "),
            "http://b/p?x=1"
        );
    }

    #[test]
    fn a_list_mutation_keeps_the_path() {
        assert_eq!(
            run(r"
                const u = new URL('http://u@a:81/p/q?x=1');
                u.searchParams.set('x', '2');
                u.href;
                "),
            "http://u@a:81/p/q?x=2"
        );
    }

    #[test]
    fn a_second_url_has_its_own_search_params() {
        assert_eq!(
            run(r"
                const a = new URL('http://a/');
                const b = new URL('http://b/');
                a.searchParams.append('x', '1');
                b.href + '|' + b.searchParams.size;
                "),
            "http://b/|0"
        );
    }

    #[rstest]
    #[case::sort(
        r"
        const u = new URL('http://a/?a=b ~');
        u.searchParams.sort();
        u.href;
        ",
        "http://a/?a=b+%7E"
    )]
    #[case::delete(
        r"
        const u = new URL('http://a/?q=%zz');
        u.searchParams.delete('none');
        u.href;
        ",
        "http://a/?q=%25zz"
    )]
    fn a_mutating_call_rewrites_a_non_canonical_query(
        #[case] source: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(run(source), expected);
    }
}
