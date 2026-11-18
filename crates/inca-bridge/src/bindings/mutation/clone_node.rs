// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `cloneNode` native.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, Exception, Result as JsResult};

use inca_node::NodeId;

use super::super::convert::{JsNodeId, throw_tree_error};
use super::super::host::Host;

/// Copies the node `node_id`, with its descendants when `deep` is true, and returns the id of the
/// copy. The copies include the content of a template. An HTML `input` copy gets the typed text of
/// its original.
pub(super) fn clone_node<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<bool>) -> JsResult<NodeId> {
    move |ctx, JsNodeId(node_id), Coerced(deep)| {
        let mut host = host.borrow_mut();
        let made = host
            .tree
            .clone_subtree(node_id, deep)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        let mut inputs = host.inputs.borrow_mut();
        for &(original, copy) in &made {
            let Some(node) = host.tree.get(copy) else {
                return Err(Exception::throw_type(&ctx, "the copy is missing"));
            };
            let typed = inputs.get(&original).map(|input| input.value().to_owned());
            if let (true, Some(typed)) = (node.is_html("input"), typed) {
                inputs.entry(copy).or_default().set_value(&typed);
            }
        }
        made.first()
            .map(|&(_, copy)| copy)
            .ok_or_else(|| Exception::throw_type(&ctx, "the copy is missing"))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::super::tests::engine_with_family;
    use crate::test_support::{engine_with_bindings, error_name};

    #[test]
    fn clone_node_returns_the_id_of_a_new_node_with_the_name_of_the_original() {
        let (engine, _host) = engine_with_family();

        let got: String = engine
            .eval(
                r"
                n.appendChild(a, t);
                const copy = n.cloneNode(a, true);
                JSON.stringify([![p, a, b, t].includes(copy), n.attributeKey(copy, 'X')]);
                ",
            )
            .unwrap();

        assert_eq!(got, r#"[true,"x"]"#);
    }

    #[test]
    fn a_deep_clone_node_copies_the_descendants() {
        let (engine, _host) = engine_with_family();

        let count: u32 = engine
            .eval(
                r"
                n.appendChild(a, t);
                n.appendChild(a, c);
                n.childNodeIds(n.cloneNode(a, true)).length;
                ",
            )
            .unwrap();

        assert_eq!(count, 2);
    }

    #[test]
    fn a_shallow_clone_node_copies_no_descendant() {
        let (engine, _host) = engine_with_family();

        let count: u32 = engine
            .eval(
                r"
                n.appendChild(a, t);
                n.appendChild(a, c);
                n.childNodeIds(n.cloneNode(a, false)).length;
                ",
            )
            .unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn a_deep_clone_node_copies_the_data_of_a_text_descendant() {
        let (engine, _host) = engine_with_family();

        let data: String = engine
            .eval(
                r"
                n.setCharacterData(t, 'abc');
                n.appendChild(a, t);
                n.textContent(n.cloneNode(a, true));
                ",
            )
            .unwrap();

        assert_eq!(data, "abc");
    }

    #[test]
    fn a_clone_node_of_the_document_is_a_document_copy() {
        let (engine, _host) = engine_with_family();

        let got: bool = engine
            .eval(
                r"
                const copy = n.cloneNode(n.documentNodeId(), true);
                copy !== n.documentNodeId() && n.childNodeIds(copy).length === 1;
                ",
            )
            .unwrap();

        assert!(got);
    }

    #[test]
    fn a_cloned_input_has_the_typed_text_of_its_original() {
        let (engine, _host) = engine_with_family();

        let texts: Vec<String> = engine
            .eval(
                r"
                const field = n.createNode('input');
                n.setInputValue(field, 'typed');
                const copy = n.cloneNode(field, false);
                [n.inputValue(copy), n.inputValue(field)];
                ",
            )
            .unwrap();

        assert_eq!(texts, ["typed", "typed"]);
    }

    #[test]
    fn a_deep_cloned_input_keeps_its_own_typed_text() {
        let (engine, _host) = engine_with_family();

        let texts: Vec<String> = engine
            .eval(
                r"
                const box = n.createNode('div');
                const one = n.createNode('input');
                const two = n.createNode('input');
                n.appendChild(box, one);
                n.appendChild(box, two);
                n.setInputValue(one, 'first');
                n.setInputValue(two, 'second');
                n.childNodeIds(n.cloneNode(box, true)).map((id) => n.inputValue(id));
                ",
            )
            .unwrap();

        assert_eq!(texts, ["first", "second"]);
    }

    #[test]
    fn clone_node_of_an_unknown_id_throws_type_error() {
        let (engine, _host) = engine_with_family();

        assert_eq!(error_name(&engine, "n.cloneNode(999, true)"), "TypeError");
    }

    #[test]
    fn clone_node_copies_the_content_fragment_of_a_template() {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const template = n.createNode('template');
                const copy = n.cloneNode(template, false);
                const content = n.templateContent(copy);
                content !== null && content !== n.templateContent(template);
                ",
            )
            .unwrap();

        assert!(got);
    }
}
