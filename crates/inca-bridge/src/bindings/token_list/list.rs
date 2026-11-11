// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `DOMTokenList` class of `classList`.
//!
//! A list holds the id of its node and reads the `class` attribute at every call. A list of a
//! freed node reads empty, and its writes change nothing.

use std::cell::RefCell;
use std::rc::Rc;

#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::atom::PredefinedAtom;
use rquickjs::convert::Coerced;
use rquickjs::function::{Rest, This};
use rquickjs::object::Property;
use rquickjs::{
    Class, Ctx, Exception, FromJs, Function, IntoJs, Object, Result as JsResult, Value,
};

use inca_jsenv::class::{
    define_method, define_this_accessor, define_this_getter, plain_class, receiver, require,
};
use inca_node::{AttributeValue, ClassListError, NodeId};

use super::iterator::{Kind, install_iterator, new_iterator};
use crate::bindings::Natives;
use crate::bindings::convert::{JsNodeId, or_null, unsigned};
use crate::bindings::host::Host;
use crate::interfaces::{define_unconstructable, interface_prototype};

/// A `DOMTokenList` instance. It holds the node id and no JS value.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
struct TokenList {
    node: NodeId,
}

plain_class!(TokenList, "DOMTokenList");

/// `classList`, and the class `DOMTokenList` with its iterator.
pub(in crate::bindings) fn install_token_list(n: &Natives<'_, '_>) -> JsResult<()> {
    let ctx = n.ctx;
    let proto = Object::new(ctx.clone())?;
    define_members(ctx, &proto, n.host)?;
    define_unconstructable(
        ctx,
        &proto,
        "DOMTokenList",
        "a token list cannot be constructed. read it from classList",
    )?;
    install_iterator(ctx, n.host)?;
    n.set("classList", class_list)
}

/// The list of the node `node_id`, which may be freed.
fn class_list<'js>(
    _host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Class<'js, TokenList>> {
    move |ctx, JsNodeId(node)| {
        let proto = interface_prototype(&ctx, "DOMTokenList")?;
        Class::instance_proto(TokenList { node }, proto)
    }
}

/// Defines `body` as the method `name` of `proto` with `length` parameters. `body` gets the
/// context, the `this` value and the arguments.
pub(in crate::bindings) fn method<'js, R: IntoJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &str,
    length: usize,
    body: impl Fn(&Ctx<'js>, Value<'js>, &[Value<'js>]) -> JsResult<R> + 'js,
) -> JsResult<Function<'js>> {
    let function = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            body(&ctx, this, &args)
        },
    )?
    .with_length(length)?;
    define_method(proto, name, function.clone())?;
    Ok(function)
}

fn node_of<'js>(ctx: &Ctx<'js>, this: &Value<'js>) -> JsResult<NodeId> {
    Ok(receiver::<TokenList>(ctx, this)?.borrow().node)
}

/// The tokens of the node, empty when the node is freed.
pub(super) fn tokens_of(host: &RefCell<Host>, node: NodeId) -> Vec<String> {
    host.borrow().tree.class_tokens(node).unwrap_or_default()
}

/// The `class` text of the node, empty when the node is freed.
fn value_of(host: &RefCell<Host>, node: NodeId) -> String {
    host.borrow().tree.class_value(node).unwrap_or_default()
}

fn text<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<String> {
    Ok(Coerced::<String>::from_js(ctx, value.clone())?.0)
}

fn texts<'js>(ctx: &Ctx<'js>, args: &[Value<'js>]) -> JsResult<Vec<String>> {
    args.iter().map(|value| text(ctx, value)).collect()
}

pub(super) fn position(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

/// The value of a write: a freed node gives the default and a bad token throws a `DOMException`.
fn written<T: Default>(ctx: &Ctx<'_>, result: Result<T, ClassListError>) -> JsResult<T> {
    match result {
        Ok(value) => Ok(value),
        Err(ClassListError::Tree(_)) => Ok(T::default()),
        Err(ClassListError::Empty) => Err(Exception::throw_dom(
            ctx,
            "SyntaxError",
            "a token is empty. pass a non-empty token",
        )),
        Err(ClassListError::Whitespace) => Err(Exception::throw_dom(
            ctx,
            "InvalidCharacterError",
            "a token contains whitespace. pass a token without whitespace",
        )),
    }
}

fn define_members<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Rc<RefCell<Host>>,
) -> JsResult<()> {
    let h = host.clone();
    define_this_getter(ctx, proto, "length", move |ctx, this| {
        Ok(position(tokens_of(&h, node_of(ctx, &this)?).len()))
    })?;
    let (read, write) = (host.clone(), host.clone());
    define_this_accessor(
        ctx,
        proto,
        "value",
        move |ctx, this| Ok(value_of(&read, node_of(ctx, &this)?)),
        move |ctx, this, Coerced(value): Coerced<String>| {
            let node = node_of(ctx, &this)?;
            let result = write.borrow_mut().tree.set_attribute(
                node,
                "class".to_owned(),
                AttributeValue::String(value),
            );
            written(ctx, result.map_err(ClassListError::Tree))
        },
    )?;
    let h = host.clone();
    method(ctx, proto, "toString", 0, move |ctx, this, _| {
        Ok(value_of(&h, node_of(ctx, &this)?))
    })?;
    let h = host.clone();
    method(ctx, proto, "item", 1, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        require(ctx, "item", args, 1)?;
        let index = unsigned(Coerced::<i32>::from_js(ctx, args[0].clone())?);
        let token = tokens_of(&h, node).into_iter().nth(index as usize);
        or_null(ctx, token)
    })?;
    let h = host.clone();
    method(ctx, proto, "contains", 1, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        require(ctx, "contains", args, 1)?;
        let token = text(ctx, &args[0])?;
        Ok(tokens_of(&h, node).contains(&token))
    })?;
    let h = host.clone();
    method(ctx, proto, "add", 0, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        let tokens = texts(ctx, args)?;
        let tokens: Vec<&str> = tokens.iter().map(String::as_str).collect();
        let result = h.borrow_mut().tree.add_classes(node, &tokens);
        written(ctx, result)
    })?;
    let h = host.clone();
    method(ctx, proto, "remove", 0, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        let tokens = texts(ctx, args)?;
        let tokens: Vec<&str> = tokens.iter().map(String::as_str).collect();
        let result = h.borrow_mut().tree.remove_classes(node, &tokens);
        written(ctx, result)
    })?;
    let h = host.clone();
    method(ctx, proto, "toggle", 1, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        require(ctx, "toggle", args, 1)?;
        let token = text(ctx, &args[0])?;
        let force = match args.get(1) {
            Some(force) if !force.is_undefined() => {
                Some(Coerced::<bool>::from_js(ctx, force.clone())?.0)
            }
            _ => None,
        };
        let result = h.borrow_mut().tree.toggle_class(node, &token, force);
        written(ctx, result)
    })?;
    let h = host.clone();
    method(ctx, proto, "replace", 2, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        require(ctx, "replace", args, 2)?;
        let (token, new_token) = (text(ctx, &args[0])?, text(ctx, &args[1])?);
        let result = h.borrow_mut().tree.replace_class(node, &token, &new_token);
        written(ctx, result)
    })?;
    method(ctx, proto, "supports", 1, |ctx, this, _| {
        node_of(ctx, &this)?;
        Err::<(), _>(Exception::throw_type(
            ctx,
            "class defines no supported tokens. use contains to look for a token",
        ))
    })?;
    define_for_each(ctx, proto, host)?;
    define_iterators(ctx, proto)
}

fn define_for_each<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Rc<RefCell<Host>>,
) -> JsResult<()> {
    let h = host.clone();
    method(ctx, proto, "forEach", 1, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        require(ctx, "forEach", args, 1)?;
        let callback = args[0].as_function().ok_or_else(|| {
            Exception::throw_type(ctx, "the callback is not a function. pass a function")
        })?;
        let this_arg = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| Value::new_undefined(ctx.clone()));
        let count = tokens_of(&h, node).len();
        for index in 0..count {
            // The borrow ends before the callback runs, so the callback may change the list.
            let Some(token) = tokens_of(&h, node).into_iter().nth(index) else {
                continue;
            };
            callback.call::<_, Value>((
                This(this_arg.clone()),
                token,
                position(index),
                this.clone(),
            ))?;
        }
        Ok(())
    })?;
    Ok(())
}

/// `keys`, `values`, `entries` and `[Symbol.iterator]`, which is the function `values`.
fn define_iterators<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    define_iterator_method(ctx, proto, "keys", Kind::Keys)?;
    let values = define_iterator_method(ctx, proto, "values", Kind::Values)?;
    define_iterator_method(ctx, proto, "entries", Kind::Entries)?;
    proto.prop(
        PredefinedAtom::SymbolIterator,
        Property::from(values).writable().configurable(),
    )
}

fn define_iterator_method<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &str,
    kind: Kind,
) -> JsResult<Function<'js>> {
    method(ctx, proto, name, 0, move |ctx, this, _| {
        new_iterator(ctx, node_of(ctx, &this)?, kind)
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::{Value, json};

    use crate::test_support::engine_with_bindings;
    use crate::tests::{list_exec, list_run};

    const EMPTY: &str = "a token is empty. pass a non-empty token";
    const WHITESPACE: &str = "a token contains whitespace. pass a token without whitespace";
    const RECEIVER: &str = "the receiver is not a DOMTokenList. call the method on an instance";
    const ITERATOR_RECEIVER: &str =
        "the receiver is not a DOMTokenList Iterator. call the method on an instance";

    fn freed(class: Option<&str>, body: &str) -> Value {
        list_exec(class, true, body)
    }

    /// The body that returns `read` after the statement `action`.
    fn after(action: &str, read: &str) -> String {
        format!(
            r"
            {action};
            return {read};
            "
        )
    }

    /// The body that returns `[name, code, isDOMException, message]` of what `call` throws.
    fn caught(call: &str) -> String {
        format!(
            r"
            try {{
                {call};
                return 'none';
            }} catch (e) {{
                return [e.name, e.code, e instanceof DOMException, e.message];
            }}
            "
        )
    }

    /// The body that returns the name of what `call` throws, or `'none'`.
    fn name_of(call: &str) -> String {
        format!(
            r"
            try {{
                {call};
                return 'none';
            }} catch (e) {{
                return e.name;
            }}
            "
        )
    }

    #[rstest]
    #[case::instance("list instanceof DOMTokenList", json!(true))]
    #[case::tag("Object.prototype.toString.call(list)", json!("[object DOMTokenList]"))]
    fn the_native_returns_an_instance_of_the_class(#[case] read: &str, #[case] expected: Value) {
        assert_eq!(list_run(None, &format!("return {read};")), expected);
    }

    #[test]
    fn the_native_returns_a_list_for_an_unknown_node() {
        let (engine, _host) = engine_with_bindings();

        let made: bool = engine
            .eval("__inca_native__.classList(99999) instanceof DOMTokenList")
            .unwrap();

        assert!(made);
    }

    #[rstest]
    #[case::symbol_iterator("Symbol.iterator", json!(false))]
    #[case::values("'values'", json!(true))]
    #[case::add("'add'", json!(true))]
    #[case::length("'length'", json!(true))]
    fn a_member_has_its_enumerable_attribute(#[case] key: &str, #[case] expected: Value) {
        let read =
            format!("Object.getOwnPropertyDescriptor(DOMTokenList.prototype, {key}).enumerable");

        assert_eq!(list_run(None, &format!("return {read};")), expected);
    }

    #[rstest]
    #[case::add_empty("list.add('')", json!(["SyntaxError", 12, true, EMPTY]))]
    #[case::add_whitespace("list.add('a b')", json!(["InvalidCharacterError", 5, true, WHITESPACE]))]
    #[case::remove_empty("list.remove('')", json!(["SyntaxError", 12, true, EMPTY]))]
    #[case::remove_whitespace("list.remove('a b')", json!(["InvalidCharacterError", 5, true, WHITESPACE]))]
    #[case::toggle_empty("list.toggle('')", json!(["SyntaxError", 12, true, EMPTY]))]
    #[case::toggle_whitespace("list.toggle('a b')", json!(["InvalidCharacterError", 5, true, WHITESPACE]))]
    #[case::replace_empty("list.replace('', 'a')", json!(["SyntaxError", 12, true, EMPTY]))]
    #[case::replace_whitespace("list.replace('a', 'a b')", json!(["InvalidCharacterError", 5, true, WHITESPACE]))]
    fn an_invalid_token_throws_a_dom_exception(#[case] call: &str, #[case] expected: Value) {
        assert_eq!(list_run(Some("a"), &caught(call)), expected);
    }

    #[rstest]
    #[case::number("list.add(1)", json!("1"))]
    #[case::undefined("list.add(undefined)", json!("undefined"))]
    fn a_token_argument_converts_to_a_string(#[case] action: &str, #[case] expected: Value) {
        assert_eq!(list_run(None, &after(action, "list.value")), expected);
    }

    #[test]
    fn a_symbol_token_throws_a_type_error() {
        assert_eq!(
            list_run(None, &name_of("list.add(Symbol())")),
            json!("TypeError")
        );
    }

    #[rstest]
    #[case::item("list.item()")]
    #[case::contains("list.contains()")]
    #[case::toggle("list.toggle()")]
    #[case::replace("list.replace('a')")]
    #[case::for_each("list.forEach()")]
    fn a_missing_argument_throws_a_type_error(#[case] call: &str) {
        assert_eq!(list_run(Some("a"), &name_of(call)), json!("TypeError"));
    }

    #[rstest]
    #[case::item("list.item", 1)]
    #[case::contains("list.contains", 1)]
    #[case::add("list.add", 0)]
    #[case::remove("list.remove", 0)]
    #[case::toggle("list.toggle", 1)]
    #[case::replace("list.replace", 2)]
    #[case::supports("list.supports", 1)]
    #[case::for_each("list.forEach", 1)]
    #[case::keys("list.keys", 0)]
    #[case::values("list.values", 0)]
    #[case::entries("list.entries", 0)]
    #[case::to_string("list.toString", 0)]
    #[case::symbol_iterator("list[Symbol.iterator]", 0)]
    fn a_method_has_its_parameter_count_as_length(#[case] method: &str, #[case] length: u32) {
        assert_eq!(
            list_run(None, &format!("return {method}.length;")),
            json!(length)
        );
    }

    #[rstest]
    #[case::length_getter(
        "Object.getOwnPropertyDescriptor(DOMTokenList.prototype, 'length').get",
        "get length"
    )]
    #[case::value_getter(
        "Object.getOwnPropertyDescriptor(DOMTokenList.prototype, 'value').get",
        "get value"
    )]
    #[case::value_setter(
        "Object.getOwnPropertyDescriptor(DOMTokenList.prototype, 'value').set",
        "set value"
    )]
    fn an_accessor_function_has_its_name(#[case] function: &str, #[case] expected: &str) {
        assert_eq!(
            list_run(None, &format!("return {function}.name;")),
            json!(expected)
        );
    }

    #[rstest]
    #[case::first("0", json!("a"))]
    #[case::second("1", json!("b"))]
    #[case::past_the_end("2", json!(null))]
    #[case::negative("-1", json!(null))]
    #[case::fraction("1.5", json!("b"))]
    #[case::numeric_string("'1'", json!("b"))]
    #[case::nan("NaN", json!("a"))]
    #[case::two_to_the_32("4294967296", json!("a"))]
    fn item_converts_the_index_to_an_unsigned_integer(
        #[case] index: &str,
        #[case] expected: Value,
    ) {
        assert_eq!(
            list_run(Some("a b"), &format!("return list.item({index});")),
            expected
        );
    }

    #[rstest]
    #[case::text(Some(" a  b "), json!(" a  b "))]
    #[case::absent(None, json!(""))]
    fn value_reads_the_attribute_text_as_stored(
        #[case] class: Option<&str>,
        #[case] expected: Value,
    ) {
        assert_eq!(list_run(class, "return list.value;"), expected);
    }

    #[test]
    fn to_string_reads_the_attribute_text_as_stored() {
        assert_eq!(
            list_run(Some("a  b"), "return list.toString();"),
            json!("a  b")
        );
    }

    #[rstest]
    #[case::text("list.value = 'x y'", json!("x y"))]
    #[case::null("list.value = null", json!("null"))]
    #[case::undefined("list.value = undefined", json!("undefined"))]
    fn the_value_setter_writes_the_attribute(#[case] action: &str, #[case] expected: Value) {
        assert_eq!(list_run(None, &after(action, "list.value")), expected);
    }

    #[rstest]
    #[case::length_after_set("n.setAttribute(id, 'class', 'a b c')", "list.length", json!(3))]
    #[case::item_after_set("n.setAttribute(id, 'class', 'a b c')", "list.item(1)", json!("b"))]
    #[case::value_after_set("n.setAttribute(id, 'class', 'a b c')", "list.value", json!("a b c"))]
    #[case::length_after_remove("n.removeAttribute(id, 'class')", "list.length", json!(0))]
    #[case::item_after_remove("n.removeAttribute(id, 'class')", "list.item(0)", json!(null))]
    #[case::value_after_remove("n.removeAttribute(id, 'class')", "list.value", json!(""))]
    fn a_list_reads_the_attribute_at_each_call(
        #[case] action: &str,
        #[case] read: &str,
        #[case] expected: Value,
    ) {
        assert_eq!(list_run(Some("a b"), &after(action, read)), expected);
    }

    #[rstest]
    #[case::omitted("list.toggle('b')", json!(true))]
    #[case::undefined("list.toggle('b', undefined)", json!(true))]
    #[case::null("list.toggle('b', null)", json!(false))]
    #[case::zero("list.toggle('b', 0)", json!(false))]
    #[case::empty_string("list.toggle('b', '')", json!(false))]
    #[case::text_false("list.toggle('b', 'false')", json!(true))]
    fn toggle_reads_force_as_a_boolean(#[case] call: &str, #[case] expected: Value) {
        assert_eq!(list_run(Some("a"), &format!("return {call};")), expected);
    }

    #[test]
    fn supports_throws_a_type_error() {
        assert_eq!(
            list_run(None, &name_of("list.supports('a')")),
            json!("TypeError")
        );
    }

    #[rstest]
    #[case::present("a", json!(true))]
    #[case::other_case("A", json!(false))]
    #[case::with_whitespace("a b", json!(false))]
    fn contains_compares_the_token_exactly(#[case] token: &str, #[case] expected: Value) {
        assert_eq!(
            list_run(Some("a"), &format!("return list.contains('{token}');")),
            expected
        );
    }

    #[test]
    fn for_each_passes_the_token_the_index_and_the_list() {
        let body = r"
            const seen = [];
            list.forEach((token, index, owner) => seen.push([token, index, owner === list]));
            return seen;
            ";

        assert_eq!(
            list_run(Some("a b"), body),
            json!([["a", 0, true], ["b", 1, true]])
        );
    }

    #[test]
    fn for_each_calls_the_callback_with_the_this_argument() {
        let body = r"
            const seen = [];
            list.forEach(function () {
                seen.push(this.tag);
            }, { tag: 'x' });
            return seen;
            ";

        assert_eq!(list_run(Some("a"), body), json!(["x"]));
    }

    #[test]
    fn for_each_throws_a_type_error_for_a_callback_that_is_not_a_function() {
        assert_eq!(
            list_run(Some("a"), &name_of("list.forEach(1)")),
            json!("TypeError")
        );
    }

    #[test]
    fn for_each_does_not_visit_a_token_added_by_the_callback() {
        let body = r"
            const seen = [];
            list.forEach((token) => {
                list.add('b');
                seen.push(token);
            });
            return seen;
            ";

        assert_eq!(list_run(Some("a"), body), json!(["a"]));
    }

    #[test]
    fn for_each_visits_the_tokens_it_started_with_when_the_callback_adds_one() {
        let body = r"
            const seen = [];
            let added = false;
            list.forEach((token) => {
                if (!added) {
                    added = true;
                    list.add('c');
                }
                seen.push(token);
            });
            return [seen, list.value];
            ";

        assert_eq!(list_run(Some("a b"), body), json!([["a", "b"], "a b c"]));
    }

    #[test]
    fn for_each_reads_the_list_live_when_the_callback_removes_a_token() {
        let body = r"
            const seen = [];
            list.forEach((token) => {
                list.remove(token);
                seen.push(token);
            });
            return seen;
            ";

        assert_eq!(list_run(Some("a b c"), body), json!(["a", "c"]));
    }

    #[test]
    fn for_each_on_the_list_of_a_freed_node_never_calls_the_callback() {
        let body = r"
            let calls = 0;
            list.forEach(() => {
                calls += 1;
            });
            return calls;
            ";

        assert_eq!(freed(Some("a b"), body), json!(0));
    }

    #[test]
    fn the_default_iterator_is_values() {
        assert_eq!(
            list_run(None, "return list[Symbol.iterator] === list.values;"),
            json!(true)
        );
    }

    #[rstest]
    #[case::method(
        "DOMTokenList.prototype.add.call({})",
        json!(["TypeError", null, false, RECEIVER])
    )]
    #[case::getter(
        "Object.getOwnPropertyDescriptor(DOMTokenList.prototype, 'length').get.call({})",
        json!(["TypeError", null, false, RECEIVER])
    )]
    #[case::iterator(
        "Object.getPrototypeOf(list.values()).next.call({})",
        json!(["TypeError", null, false, ITERATOR_RECEIVER])
    )]
    fn a_foreign_receiver_throws_a_type_error_that_names_the_class(
        #[case] call: &str,
        #[case] expected: Value,
    ) {
        assert_eq!(list_run(None, &caught(call)), expected);
    }

    #[rstest]
    #[case::length("list.length", json!(0))]
    #[case::value("list.value", json!(""))]
    #[case::item("list.item(0)", json!(null))]
    #[case::contains("list.contains('a')", json!(false))]
    #[case::iteration("[...list]", json!([]))]
    fn a_list_of_a_freed_node_reads_empty(#[case] call: &str, #[case] expected: Value) {
        assert_eq!(freed(Some("a b"), &format!("return {call};")), expected);
    }

    #[rstest]
    #[case::add("list.add('x')")]
    #[case::remove("list.remove('a')")]
    #[case::value("list.value = 'x'")]
    fn a_write_to_a_list_of_a_freed_node_is_ignored(#[case] call: &str) {
        assert_eq!(freed(Some("a b"), &name_of(call)), json!("none"));
    }

    #[rstest]
    #[case::toggle("list.toggle('a')")]
    #[case::replace("list.replace('a', 'b')")]
    fn a_boolean_write_to_a_list_of_a_freed_node_returns_false(#[case] call: &str) {
        assert_eq!(freed(Some("a b"), &format!("return {call};")), json!(false));
    }

    #[rstest]
    #[case::empty("list.add('')", "SyntaxError")]
    #[case::whitespace("list.replace('a', 'a b')", "InvalidCharacterError")]
    fn a_list_of_a_freed_node_still_checks_its_tokens(#[case] call: &str, #[case] name: &str) {
        assert_eq!(freed(Some("a b"), &name_of(call)), json!(name));
    }
}
