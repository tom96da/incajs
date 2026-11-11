// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `URL` class.

use std::rc::Rc;

use rquickjs::function::{IntoJsFunc, Opt, ParamRequirement, Params, Rest, This};
use rquickjs::{Class, Ctx, Exception, Function, Object, Result as JsResult, Value};
use url::{Url, quirks};

use super::{ParamsInstance, STORE, State, UrlInstance, given, receiver, require, url_state, usv};
use crate::class::{
    NEEDS_NEW, define_accessor, define_class, define_getter, define_global, define_method,
    new_target_prototype,
};
use crate::vault::vault;

const INVALID_URL: &str =
    "the URL is not valid. pass an absolute URL, or a relative one with a base";
const INVALID_HREF: &str = "the href is not valid. set an absolute URL";
const INVALID_BASE: &str = "the base URL is not valid. pass an absolute URL as the base";

/// `url` and `base` converted from the arguments of a parse.
fn parse_arguments(
    ctx: &Ctx<'_>,
    method: &str,
    args: &[Value<'_>],
) -> JsResult<(String, Option<String>)> {
    require(ctx, method, args, 1)?;
    let url = usv(&args[0])?;
    let base = given(args, 1).map(usv).transpose()?;
    Ok((url, base))
}

/// The URL `url` names against `base`, or the message of the `TypeError` that says what fails.
fn parse(url: &str, base: Option<&str>) -> Result<Url, &'static str> {
    match base {
        None => Url::parse(url).map_err(|_| INVALID_URL),
        Some(base) => Url::parse(base)
            .map_err(|_| INVALID_BASE)?
            .join(url)
            .map_err(|_| INVALID_URL),
    }
}

/// A `URL` instance for `url` with the prototype `proto`.
fn instance<'js>(
    ctx: &Ctx<'js>,
    url: Url,
    proto: Object<'js>,
) -> JsResult<Class<'js, UrlInstance<'js>>> {
    let state = State::new(Some(url), Vec::new());
    state.borrow_mut().rebuild();
    let store: Object = vault(ctx)?.get(STORE)?;
    let params = Class::instance_proto(ParamsInstance(Rc::clone(&state)), store.get("params")?)?;
    Class::instance_proto(UrlInstance { state, params }, proto)
}

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
        let args: Vec<_> = (0..params.len()).filter_map(|i| params.arg(i)).collect();
        let (url, base) = parse_arguments(ctx, "URL", &args)?;
        let url =
            parse(&url, base.as_deref()).map_err(|message| Exception::throw_type(ctx, message))?;
        let store: Object = vault(ctx)?.get(STORE)?;
        let proto = new_target_prototype(&params, store.get("url")?)?;
        Ok(instance(ctx, url, proto)?.into_value())
    }
}

type Read = fn(&Url) -> String;
type Write = fn(&mut Url, &str);

/// A setter whose refusal the standard ignores.
macro_rules! ignoring {
    ($set:ident) => {
        |url, value| {
            let _ = quirks::$set(url, value);
        }
    };
}

/// A read of the string a component of the URL holds.
macro_rules! component {
    ($get:ident) => {
        |url| quirks::$get(url).to_owned()
    };
}

fn read_href(url: &Url) -> String {
    quirks::href(url).to_owned()
}

fn read_search(url: &Url) -> String {
    quirks::search(url).to_owned()
}

/// The `origin` of a `file:` URL reads `file://`.
fn read_origin(url: &Url) -> String {
    if url.scheme() == "file" {
        "file://".to_owned()
    } else {
        quirks::origin(url)
    }
}

/// The string members of a `URL` in print order: name and read.
pub(super) const MEMBERS: [(&str, Read); 11] = [
    ("href", read_href),
    ("origin", read_origin),
    ("protocol", component!(protocol)),
    ("username", component!(username)),
    ("password", component!(password)),
    ("host", component!(host)),
    ("hostname", component!(hostname)),
    ("port", component!(port)),
    ("pathname", component!(pathname)),
    ("search", read_search),
    ("hash", component!(hash)),
];

/// The components that write the URL alone: name, read and setter.
const COMPONENTS: [(&str, Read, Write); 8] = [
    ("protocol", component!(protocol), ignoring!(set_protocol)),
    ("username", component!(username), ignoring!(set_username)),
    ("password", component!(password), ignoring!(set_password)),
    ("host", component!(host), ignoring!(set_host)),
    ("hostname", component!(hostname), ignoring!(set_hostname)),
    ("port", component!(port), ignoring!(set_port)),
    ("pathname", component!(pathname), quirks::set_pathname),
    ("hash", component!(hash), quirks::set_hash),
];

/// A getter that returns what `read` makes of the URL.
fn getter<'js>(ctx: &Ctx<'js>, read: Read) -> JsResult<Function<'js>> {
    Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>| -> JsResult<String> {
            let state = url_state(&ctx, &this)?;
            let state = state.borrow();
            Ok(state.url.as_ref().map(read).unwrap_or_default())
        },
    )
}

/// A setter that converts its value, then applies `apply`. A refusal throws a `TypeError`.
fn setter<'js>(
    ctx: &Ctx<'js>,
    apply: impl Fn(&mut State, &str) -> Result<(), ()> + 'js,
) -> JsResult<Function<'js>> {
    Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>,
              This(this): This<Value<'js>>,
              value: Opt<Value<'js>>|
              -> JsResult<()> {
            let state = url_state(&ctx, &this)?;
            let value = value.0.unwrap_or_else(|| Value::new_undefined(ctx.clone()));
            let value = usv(&value)?;
            let applied = apply(&mut state.borrow_mut(), &value);
            applied.map_err(|()| Exception::throw_type(&ctx, INVALID_HREF))
        },
    )
}

pub(super) fn define_url<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    let class = define_class(ctx, proto, "URL", 1, Construct)?;
    let can_parse = Function::new(
        ctx.clone(),
        |ctx: Ctx<'js>, Rest(args): Rest<Value<'js>>| {
            let (url, base) = parse_arguments(&ctx, "URL.canParse", &args)?;
            Ok::<_, rquickjs::Error>(parse(&url, base.as_deref()).is_ok())
        },
    )?;
    let parse_url = Function::new(
        ctx.clone(),
        |ctx: Ctx<'js>, Rest(args): Rest<Value<'js>>| {
            let (url, base) = parse_arguments(&ctx, "URL.parse", &args)?;
            let Ok(url) = parse(&url, base.as_deref()) else {
                return Ok(Value::new_null(ctx));
            };
            let store: Object = vault(&ctx)?.get(STORE)?;
            Ok::<_, rquickjs::Error>(instance(&ctx, url, store.get("url")?)?.into_value())
        },
    )?;
    for (name, function) in [("canParse", can_parse), ("parse", parse_url)] {
        define_method(&class, name, function.with_length(1)?)?;
    }
    for (name, read, write) in COMPONENTS {
        let set = setter(ctx, move |state, value| {
            if let Some(url) = &mut state.url {
                write(url, value);
            }
            Ok(())
        })?;
        define_accessor(proto, name, getter(ctx, read)?, set)?;
    }
    define_accessor(
        proto,
        "href",
        getter(ctx, read_href)?,
        setter(ctx, State::set_href)?,
    )?;
    define_accessor(
        proto,
        "search",
        getter(ctx, read_search)?,
        setter(ctx, |state, value| {
            state.set_search(value);
            Ok(())
        })?,
    )?;
    define_getter(proto, "origin", getter(ctx, read_origin)?)?;
    define_getter(
        proto,
        "searchParams",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(this): This<Value<'js>>| -> JsResult<Class<'js, ParamsInstance>> {
                let class = receiver::<UrlInstance>(&ctx, &this)?;
                let params = class.borrow().params.clone();
                Ok(params)
            },
        )?,
    )?;
    for name in ["toString", "toJSON"] {
        define_method(proto, name, getter(ctx, read_href)?)?;
    }
    define_global(ctx, "URL", class)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::{INVALID_BASE, INVALID_HREF, INVALID_URL};
    use crate::Engine;
    use crate::tests::{around, run, thrown};

    const SAMPLE: &str = "https://u:p@a.example:8080/x/y?q=1#f";

    #[rstest]
    #[case::getter("Object.getOwnPropertyDescriptor(URL.prototype, 'href').get.call({})")]
    #[case::setter(
        "Object.getOwnPropertyDescriptor(URL.prototype, 'href').set.call({}, 'http://b/')"
    )]
    #[case::to_string("URL.prototype.toString.call({})")]
    #[case::to_json("URL.prototype.toJSON.call({})")]
    #[case::search_params(
        "Object.getOwnPropertyDescriptor(URL.prototype, 'searchParams').get.call({})"
    )]
    fn a_member_on_a_wrong_receiver_throws(#[case] call: &str) {
        assert_eq!(
            thrown(call),
            "TypeError: the receiver is not a URL. call the method on an instance"
        );
    }

    #[test]
    fn url_exposes_no_own_keys() {
        assert_eq!(
            run("String(Reflect.ownKeys(new URL('http://a/?x=1')).length)"),
            "0"
        );
    }

    #[rstest]
    #[case::href("href", "-ecgs")]
    #[case::origin("origin", "-ecg-")]
    #[case::search_params("searchParams", "-ecg-")]
    fn url_accessors_have_their_attributes(#[case] key: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!(
                r"
                const d = Object.getOwnPropertyDescriptor(URL.prototype, '{key}');
                [
                    d.writable ? 'w' : '-',
                    d.enumerable ? 'e' : '-',
                    d.configurable ? 'c' : '-',
                    d.get ? 'g' : '-',
                    d.set ? 's' : '-',
                ].join('');
                "
            )),
            expected
        );
    }

    #[rstest]
    #[case::can_parse("canParse")]
    #[case::parse("parse")]
    fn the_static_methods_are_writable_enumerable_and_configurable(#[case] name: &str) {
        assert_eq!(
            run(&format!(
                r"
                const d = Object.getOwnPropertyDescriptor(URL, '{name}');
                [d.writable, d.enumerable, d.configurable].join();
                "
            )),
            "true,true,true"
        );
    }

    #[rstest]
    #[case::can_parse("canParse")]
    #[case::parse("parse")]
    fn the_static_methods_have_length_one(#[case] name: &str) {
        assert_eq!(run(&format!("String(URL.{name}.length)")), "1");
    }

    #[test]
    fn the_constructor_resolves_a_relative_url_against_a_base() {
        assert_eq!(
            run("new URL('../a b?x=1#h', 'http://example.com/d/e/f').href"),
            "http://example.com/d/a%20b?x=1#h"
        );
    }

    #[test]
    fn the_constructor_converts_an_object_with_to_string() {
        assert_eq!(
            run(r"
                const input = {
                    toString() {
                        return 'http://a/';
                    },
                };
                new URL(input).href;
                "),
            "http://a/"
        );
    }

    #[test]
    fn the_constructor_treats_an_undefined_base_as_absent() {
        assert_eq!(run("new URL('http://a/p', undefined).href"), "http://a/p");
    }

    #[rstest]
    #[case::url("new URL('nope')", INVALID_URL)]
    #[case::relative_without_base("new URL('/p', undefined)", INVALID_URL)]
    #[case::base("new URL('/p', 'nope')", INVALID_BASE)]
    #[case::null_base("new URL('/p', null)", INVALID_BASE)]
    fn the_constructor_throws_a_type_error_for_an_invalid_url(
        #[case] source: &str,
        #[case] message: &str,
    ) {
        assert_eq!(thrown(source), format!("TypeError: {message}"));
    }

    #[test]
    fn the_constructor_throws_without_arguments() {
        assert_eq!(
            thrown("new URL()"),
            "TypeError: too few arguments for URL. pass at least 1"
        );
    }

    #[test]
    fn the_constructor_replaces_a_lone_surrogate_with_u_fffd() {
        assert_eq!(run(r"new URL('http://a/\uD800').pathname"), "/%EF%BF%BD");
    }

    #[rstest]
    #[case::absolute("URL.canParse('http://a/')", "true")]
    #[case::relative_with_base("URL.canParse('/p', 'http://a')", "true")]
    #[case::invalid("URL.canParse('x')", "false")]
    #[case::invalid_base("URL.canParse('/p', 'bad')", "false")]
    fn can_parse_answers_without_throwing(#[case] source: &str, #[case] expected: &str) {
        assert_eq!(run(&format!("String({source})")), expected);
    }

    #[test]
    fn can_parse_without_arguments_throws() {
        assert_eq!(
            thrown("URL.canParse()"),
            "TypeError: too few arguments for URL.canParse. pass at least 1"
        );
    }

    #[rstest]
    #[case::url("String(URL.parse('http://a/') instanceof URL)", "true")]
    #[case::null("String(URL.parse('x') === null)", "true")]
    fn parse_returns_a_url_or_null(#[case] source: &str, #[case] expected: &str) {
        assert_eq!(run(source), expected);
    }

    #[rstest]
    #[case::href("href", SAMPLE)]
    #[case::protocol("protocol", "https:")]
    #[case::username("username", "u")]
    #[case::password("password", "p")]
    #[case::host("host", "a.example:8080")]
    #[case::hostname("hostname", "a.example")]
    #[case::port("port", "8080")]
    #[case::pathname("pathname", "/x/y")]
    #[case::search("search", "?q=1")]
    #[case::hash("hash", "#f")]
    fn a_getter_returns_its_component(#[case] member: &str, #[case] expected: &str) {
        assert_eq!(run(&format!("new URL('{SAMPLE}').{member}")), expected);
    }

    #[rstest]
    #[case::username("http://a/", "username")]
    #[case::password("http://a/", "password")]
    #[case::port("http://a/", "port")]
    #[case::opaque_hostname("mailto:x", "hostname")]
    #[case::empty_search("http://a/?#", "search")]
    #[case::empty_hash("http://a/?#", "hash")]
    fn a_getter_returns_an_empty_string_for_an_absent_part(
        #[case] url: &str,
        #[case] member: &str,
    ) {
        assert_eq!(run(&format!("new URL('{url}').{member}")), "");
    }

    #[test]
    fn the_host_omits_the_default_port() {
        assert_eq!(run("new URL('http://a:80/').host"), "a");
    }

    #[rstest]
    #[case::tuple("http://a:80/x", "http://a")]
    #[case::port("http://a:81/x", "http://a:81")]
    #[case::opaque("data:,x", "null")]
    fn origin_is_the_serialized_tuple_or_null(#[case] url: &str, #[case] expected: &str) {
        assert_eq!(run(&format!("new URL('{url}').origin")), expected);
    }

    #[rstest]
    #[case::empty_host("file:///a")]
    #[case::host("file://h/x")]
    fn origin_of_a_file_url_is_file_slashes(#[case] url: &str) {
        assert_eq!(run(&format!("new URL('{url}').origin")), "file://");
    }

    #[rstest]
    #[case::protocol_special("http://a:8/", "protocol", "https", "https://a:8/")]
    #[case::protocol_ws("ws://a:8/", "protocol", "wss:x", "wss://a:8/")]
    #[case::protocol_invalid("http://a/", "protocol", "1x", "http://a/")]
    #[case::host_keeps_port("http://a:7/p", "host", "b", "http://b:7/p")]
    #[case::host_with_port("http://a:7/p", "host", "b:81", "http://b:81/p")]
    #[case::hostname_with_port("http://a:7/p", "hostname", "b:81", "http://a:7/p")]
    #[case::username("http://a/", "username", "us er@:", "http://us%20er%40%3A@a/")]
    #[case::password("http://a/", "password", "p/w d", "http://:p%2Fw%20d@a/")]
    #[case::port_clears("http://a:8/", "port", "", "http://a/")]
    #[case::port_prefix("http://a/", "port", "8080abc", "http://a:8080/")]
    #[case::port_too_large("http://a:8/", "port", "65536", "http://a:8/")]
    #[case::pathname("http://a/", "pathname", "x y", "http://a/x%20y")]
    #[case::pathname_backslash("http://a/", "pathname", "\\\\a\\\\b", "http://a/a/b")]
    #[case::search("http://a/", "search", "?a b", "http://a/?a%20b")]
    #[case::search_second_mark("http://a/", "search", "??a", "http://a/??a")]
    #[case::search_empty_keeps_fragment("http://a/?x#f", "search", "", "http://a/#f")]
    #[case::hash("http://a/", "hash", "#x", "http://a/#x")]
    #[case::hash_without_mark("http://a/", "hash", "x", "http://a/#x")]
    #[case::hash_lone_mark("http://a/", "hash", "#", "http://a/#")]
    #[case::hash_empty("http://a/#x", "hash", "", "http://a/")]
    fn a_setter_follows_its_algorithm(
        #[case] start: &str,
        #[case] member: &str,
        #[case] value: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            run(&format!(
                r"
                const u = new URL('{start}');
                u.{member} = '{value}';
                u.href;
                "
            )),
            expected
        );
    }

    #[rstest]
    #[case::special_to_opaque("http://a/", "protocol", "mailto")]
    #[case::file_username("file:///a", "username", "u")]
    #[case::file_port("file:///a", "port", "81")]
    #[case::opaque_pathname("mailto:x", "pathname", "y")]
    #[case::opaque_hostname("mailto:x", "hostname", "y")]
    #[case::file_with_credentials("http://u@a/", "protocol", "file")]
    fn an_ignored_setter_leaves_the_url_unchanged(
        #[case] start: &str,
        #[case] member: &str,
        #[case] value: &str,
    ) {
        assert_eq!(
            run(&format!(
                r"
                const u = new URL('{start}');
                u.{member} = '{value}';
                String(u.href === '{start}');
                "
            )),
            "true"
        );
    }

    #[rstest]
    #[case::port("port", "8080", "http://a:8080/")]
    #[case::hash("hash", "undefined", "http://a/#undefined")]
    fn a_setter_converts_a_non_string(
        #[case] member: &str,
        #[case] value: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            run(&format!(
                r"
                const u = new URL('http://a/');
                u.{member} = {value};
                u.href;
                "
            )),
            expected
        );
    }

    #[test]
    fn the_href_setter_replaces_the_url() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/');
                u.href = 'http://b/p?q#f';
                u.href;
                "),
            "http://b/p?q#f"
        );
    }

    #[test]
    fn the_href_setter_throws_for_an_invalid_url() {
        assert_eq!(
            thrown("new URL('http://a/').href = 'nope'"),
            format!("TypeError: {INVALID_HREF}")
        );
    }

    #[test]
    fn the_href_setter_keeps_the_url_when_it_throws() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/');
                try {
                    u.href = 'nope';
                } catch (e) {}
                u.href;
                "),
            "http://a/"
        );
    }

    #[rstest]
    #[case::to_string("String(new URL('http://a'))")]
    #[case::to_json("new URL('http://a').toJSON()")]
    #[case::json_stringify("JSON.parse(JSON.stringify(new URL('http://a')))")]
    fn a_url_serializes_to_its_href(#[case] source: &str) {
        assert_eq!(run(source), "http://a/");
    }

    #[test]
    fn a_conversion_that_changes_the_url_does_not_panic() {
        assert_eq!(
            run(r"
                const u = new URL('http://a/');
                u.hash = {
                    toString() {
                        u.href = 'http://b/';
                        return 'x';
                    },
                };
                u.href;
                "),
            "http://b/#x"
        );
    }

    #[test]
    fn creating_and_dropping_urls_leaves_the_counters_at_the_baseline() {
        let engine = Engine::new().unwrap();
        let (before, after) = around(&engine, |ctx| {
            ctx.eval::<(), _>(
                r"
                {
                    const u = new URL('http://a/?x=1');
                    u.searchParams.keys().next();
                    const v = new URL('http://a/?x=1');
                    v.searchParams.back = v;
                }
                ",
            )
            .unwrap();
        });

        assert_eq!(before, after);
    }
}
