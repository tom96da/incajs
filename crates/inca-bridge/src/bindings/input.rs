// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reading and writing the text of an `input` node.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, Result as JsResult};

use inca_gpui::NodeId;

use super::Natives;
use super::convert::{JsNodeId, existing_node};
use super::host::Host;

pub(super) fn install_input(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("inputValue", input_value)?;
    n.set("setInputValue", set_input_value)
}

/// Throws a `TypeError` unless `id` is an `input` node of the tree of `host`.
/// `verb` names what the caller does with the value.
fn input_node(ctx: &Ctx<'_>, host: &Host, id: NodeId, verb: &str) -> JsResult<()> {
    let node = existing_node(ctx, &host.tree, id)?;
    if node.tag_name() == "input" {
        return Ok(());
    }
    let message = format!("node {id} is not an input. only an input element has a value to {verb}");
    Err(Exception::throw_type(ctx, &message))
}

fn input_value<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<String> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.borrow();
        input_node(&ctx, &host, node_id, "read")?;
        let inputs = host.inputs.borrow();
        Ok(inputs
            .get(&node_id)
            .map_or_else(String::new, |input| input.value().to_owned()))
    }
}

fn set_input_value<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), value| {
        let host = host.borrow();
        input_node(&ctx, &host, node_id, "write")?;
        host.inputs
            .borrow_mut()
            .entry(node_id)
            .or_default()
            .set_value(&value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::test_support::engine_with_bindings;

    #[test]
    fn input_value_reads_what_set_input_value_wrote() {
        let (engine, _host) = engine_with_bindings();

        let read: Vec<String> = engine
            .eval(
                r"
                const node = __inca_native__.createNode('input');
                const before = __inca_native__.inputValue(node);
                __inca_native__.setInputValue(node, 'a\nb');
                [before, __inca_native__.inputValue(node)];
                ",
            )
            .unwrap();

        assert_eq!(read, ["", "ab"]);
    }

    #[rstest]
    #[case::read("__inca_native__.inputValue(node)")]
    #[case::write("__inca_native__.setInputValue(node, 'x')")]
    fn an_input_native_on_another_tag_throws(#[case] call: &str) {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(&format!(
                r"
                const node = __inca_native__.createNode('div');
                let caught = false;
                try {{
                    {call};
                }} catch (e) {{
                    caught = e instanceof TypeError;
                }}
                caught;
                "
            ))
            .unwrap();

        assert!(caught, "{call}");
    }

    #[test]
    fn an_input_native_on_a_missing_node_throws() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.inputValue(999);
                } catch (e) {
                    caught = e instanceof TypeError;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught);
    }
}
