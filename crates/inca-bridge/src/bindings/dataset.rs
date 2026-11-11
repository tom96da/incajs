// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Natives that read, write, delete and list the `data-*` attributes of a node for `dataset`.
//!
//! A key that no `data-*` name maps to reads `undefined` and deletes nothing. A freed node reads
//! as an empty dataset and ignores writes. A write still checks its name, so it throws for a
//! name that is invalid on a freed node too.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, IntoJs, Object, Result as JsResult, Value};

use inca_node::{AttributeValue, VirtualNode, data_attribute, valid_attribute_name};

use super::Natives;
use super::convert::{JsNodeId, throw_tree_error};
use super::host::Host;
use crate::interfaces::interface_prototype;

/// `datasetGet`, `datasetSet`, `datasetDelete`, `datasetKeys` and `datasetTarget`: the natives
/// behind the `dataset` proxy of an element.
pub(super) fn install_dataset(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("datasetGet", dataset_get)?;
    n.set("datasetSet", dataset_set)?;
    n.set("datasetDelete", dataset_delete)?;
    n.set("datasetKeys", dataset_keys)?;
    n.set("datasetTarget", dataset_target)
}

/// The value of the attribute `key` of the node `node_id` as text, `undefined` when the attribute
/// is absent or the key is not supported.
fn dataset_get<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), key| {
        let host = host.borrow();
        let Ok(name) = data_attribute(&key) else {
            return Ok(Value::new_undefined(ctx));
        };
        let Some(value) = host
            .tree
            .get(node_id)
            .and_then(|node| node.attributes().get(&name))
        else {
            return Ok(Value::new_undefined(ctx));
        };
        value.text().into_js(&ctx)
    }
}

/// Writes the attribute of `key` with the text `value`. A key with a dash before a lower case
/// letter throws a `SyntaxError`, and an invalid attribute name throws an `InvalidCharacterError`.
/// A freed node ignores the write.
fn dataset_set<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, String) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key, value| {
        let name = data_attribute(&key).map_err(|_| {
            Exception::throw_dom(
                &ctx,
                "SyntaxError",
                concat!(
                    "a dataset key cannot hold a dash before a lower case letter. ",
                    "write the key in camelCase"
                ),
            )
        })?;
        if !valid_attribute_name(&name) {
            return Err(Exception::throw_dom(
                &ctx,
                "InvalidCharacterError",
                concat!(
                    "the key makes an attribute name that is not valid. ",
                    "remove spaces and the characters / = >"
                ),
            ));
        }
        let mut host = host.borrow_mut();
        if host.tree.get(node_id).is_none() {
            return Ok(());
        }
        host.tree
            .set_attribute(node_id, name, AttributeValue::String(value))
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

/// Removes the attribute of `key`. Returns `true` in every case, also for an absent or unsupported
/// key and a freed node.
fn dataset_delete<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<bool> {
    move |_ctx, JsNodeId(node_id), key| {
        let Ok(name) = data_attribute(&key) else {
            return Ok(true);
        };
        let mut host = host.borrow_mut();
        if host.tree.get(node_id).is_some() {
            // The node exists here, so the removal cannot fail and its result is dropped.
            let _ = host.tree.remove_attribute(node_id, &name);
        }
        Ok(true)
    }
}

/// The dataset keys of the node `node_id` in attribute order, empty for a freed node.
fn dataset_keys<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Vec<String>> {
    move |_ctx, JsNodeId(node_id)| {
        Ok(host
            .borrow()
            .tree
            .get(node_id)
            .map(VirtualNode::dataset_keys)
            .unwrap_or_default())
    }
}

/// A new plain object whose prototype is the `DOMStringMap` prototype. The proxy of `dataset`
/// uses it as its target.
fn dataset_target<'js>(_host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>) -> JsResult<Object<'js>> {
    move |ctx| {
        let target = Object::new(ctx.clone())?;
        target.set_prototype(Some(&interface_prototype(&ctx, "DOMStringMap")?))?;
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::{engine_with_bindings, error_name};
    use inca_node::NodeId;
    use rstest::rstest;

    #[rstest]
    #[case::absent_key("fooBar")]
    #[case::unsupported_key("foo-bar")]
    fn get_of_a_key_without_an_attribute_is_undefined(#[case] key: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.datasetGet(node, '{key}') === undefined;
                "
            ))
            .unwrap();

        assert!(got);
    }

    #[rstest]
    #[case::number("3", "'3'")]
    #[case::boolean("true", "'true'")]
    fn get_reads_a_number_or_a_boolean_attribute_as_text(#[case] value: &str, #[case] text: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'data-x', {value});
                n.datasetGet(node, 'x') === {text};
                "
            ))
            .unwrap();

        assert!(got);
    }

    #[test]
    fn set_writes_the_attribute_that_get_reads_back() {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.datasetSet(node, 'fooBar', 'v');
                n.getAttribute(node, 'data-foo-bar') === 'v'
                    && n.datasetGet(node, 'fooBar') === 'v';
                ",
            )
            .unwrap();

        assert!(got);
    }

    #[rstest]
    #[case::empty_key("", "data-")]
    #[case::digit_after_dash("foo-1", "data-foo-1")]
    #[case::upper_case("FooBar", "data--foo-bar")]
    fn set_writes_the_attribute_its_key_names(#[case] key: &str, #[case] attribute: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.datasetSet(node, '{key}', 'v');
                n.getAttribute(node, '{attribute}') === 'v';
                "
            ))
            .unwrap();

        assert!(got);
    }

    #[test]
    fn set_throws_syntax_error_for_a_dash_before_a_lower_case_letter() {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(
                &engine,
                "__inca_native__.datasetSet(__inca_native__.createNode('div'), 'foo-bar', 'x')",
            ),
            "SyntaxError"
        );
    }

    #[rstest]
    #[case::space("a b")]
    #[case::slash("a/b")]
    #[case::equals("a=b")]
    #[case::gt("a>b")]
    #[case::nul(r"a\0b")]
    #[case::tab(r"a\tb")]
    #[case::newline(r"a\nb")]
    fn set_throws_invalid_character_for_an_invalid_name(#[case] key: &str) {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(
                &engine,
                &format!(
                    "__inca_native__.datasetSet(__inca_native__.createNode('div'), '{key}', 'x')"
                ),
            ),
            "InvalidCharacterError"
        );
    }

    #[test]
    fn delete_removes_the_attribute_and_returns_true() {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'data-x', 'v');
                n.datasetDelete(node, 'x') === true && n.getAttribute(node, 'data-x') === null;
                ",
            )
            .unwrap();

        assert!(got);
    }

    #[rstest]
    #[case::absent_key("nothing")]
    #[case::unsupported_key("foo-bar")]
    fn delete_of_a_key_without_an_attribute_returns_true(#[case] key: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.datasetDelete(node, '{key}') === true;
                "
            ))
            .unwrap();

        assert!(got);
    }

    #[test]
    fn keys_follow_the_attribute_order_and_skip_names_without_a_key() {
        let (engine, _host) = engine_with_bindings();

        let keys: Vec<String> = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'data-b', '1');
                n.setAttribute(node, 'id', 'x');
                n.setAttribute(node, 'data-a-c', '2');
                n.datasetKeys(node);
                ",
            )
            .unwrap();

        assert_eq!(keys, ["b", "aC"]);
    }

    #[test]
    fn dataset_target_takes_the_dom_string_map_prototype() {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(
                r"
                Object.getPrototypeOf(__inca_native__.datasetTarget()) === DOMStringMap.prototype;
                ",
            )
            .unwrap();

        assert!(got);
    }

    #[rstest]
    #[case::get("__inca_native__.datasetGet(NODE, 'x') === undefined")]
    #[case::delete("__inca_native__.datasetDelete(NODE, 'x') === true")]
    #[case::keys("__inca_native__.datasetKeys(NODE).length === 0")]
    fn a_freed_node_reads_no_key_and_deletes_nothing(#[case] check: &str) {
        let (engine, host) = engine_with_bindings();
        let node: NodeId = engine.eval("__inca_native__.createNode('div')").unwrap();
        host.borrow_mut().sweep_orphans().unwrap();

        let got: bool = engine
            .eval(&check.replace("NODE", &node.to_string()))
            .unwrap();

        assert!(got);
    }

    #[test]
    fn a_freed_node_ignores_a_write_of_a_valid_key() {
        let (engine, host) = engine_with_bindings();
        let node: NodeId = engine.eval("__inca_native__.createNode('div')").unwrap();
        host.borrow_mut().sweep_orphans().unwrap();

        engine
            .eval::<()>(&format!("__inca_native__.datasetSet({node}, 'x', 'v')"))
            .unwrap();
    }

    #[test]
    fn a_freed_node_still_throws_for_an_invalid_name() {
        let (engine, host) = engine_with_bindings();
        let node: NodeId = engine.eval("__inca_native__.createNode('div')").unwrap();
        host.borrow_mut().sweep_orphans().unwrap();

        assert_eq!(
            error_name(
                &engine,
                &format!("__inca_native__.datasetSet({node}, 'a b', 'x')"),
            ),
            "InvalidCharacterError"
        );
    }
}
