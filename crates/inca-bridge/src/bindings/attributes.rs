// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Natives that read and write the attributes and styles of a node.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, IntoJs, Result as JsResult, Value};

use inca_gpui::{AttributeValue, NodeId, VirtualNode, style_warning};

use super::convert::{JsNodeId, existing_node, or_null, throw_tree_error};
use super::host::Host;
use super::tree::mark_focus_dirty;

pub(super) fn set_attribute<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, Value<'js>) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key, value| {
        let value = attribute_value_from_js(&ctx, &value)?;
        let mut host = host.borrow_mut();
        let key = key.to_ascii_lowercase();
        host.tree
            .set_attribute(node_id, key.clone(), value)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        mark_focus_dirty_for(&mut host, node_id, &key);
        Ok(())
    }
}

pub(super) fn get_attribute<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), key| {
        let key = key.to_ascii_lowercase();
        let host = host.borrow();
        let node = existing_node(&ctx, &host.tree, node_id)?;
        attribute_value_to_js(&ctx, node.attributes().get(&key))
    }
}

pub(super) fn remove_attribute<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key| {
        let mut host = host.borrow_mut();
        let key = key.to_ascii_lowercase();
        host.tree
            .remove_attribute(node_id, &key)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        mark_focus_dirty_for(&mut host, node_id, &key);
        Ok(())
    }
}

pub(super) fn attribute_names<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Vec<String>> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.borrow();
        let node = existing_node(&ctx, &host.tree, node_id)?;
        let mut names: Vec<String> = node.attributes().keys().cloned().collect();
        names.sort_unstable();
        Ok(names)
    }
}

pub(super) fn set_style<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, Value<'js>) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key, value| {
        let value = attribute_value_from_js(&ctx, &value)?;
        let mut host = host.borrow_mut();
        let warning = style_warning(&key, &value);
        let is_display = key == "display";
        host.tree
            .set_style(node_id, key, value)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        if is_display {
            mark_focus_dirty(&mut host, node_id);
        }
        if let Some(warning) = warning
            && host
                .style_warned
                .entry(node_id)
                .or_default()
                .insert(warning.clone())
        {
            let tag = host.tree.get(node_id).map_or("", VirtualNode::tag_name);
            (host.warn)(&format!("node {node_id} ({tag}): {warning}"));
        }
        Ok(())
    }
}

pub(super) fn remove_style<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key| {
        let mut host = host.borrow_mut();
        host.tree
            .remove_style(node_id, &key)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        if key == "display" {
            mark_focus_dirty(&mut host, node_id);
        }
        Ok(())
    }
}

/// Queues the focus re-check an attribute change needs.
fn mark_focus_dirty_for(host: &mut Host, node_id: NodeId, key: &str) {
    match key {
        "tabindex" | "disabled" => host.focus.mark_tab_dirty(node_id),
        "hidden" | "inert" => mark_focus_dirty(host, node_id),
        _ => {}
    }
}

fn attribute_value_from_js<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<AttributeValue> {
    if value.is_string() {
        return value.get::<String>().map(AttributeValue::String);
    }
    if value.is_number() {
        return value.get::<f64>().map(AttributeValue::Number);
    }
    if value.is_bool() {
        return value.get::<bool>().map(AttributeValue::Bool);
    }
    Err(Exception::throw_type(
        ctx,
        "value must be a string, number, or boolean",
    ))
}

/// A missing key converts to `null`.
fn attribute_value_to_js<'js>(
    ctx: &Ctx<'js>,
    value: Option<&AttributeValue>,
) -> JsResult<Value<'js>> {
    let value = value
        .map(|value| match value {
            AttributeValue::String(s) => s.as_str().into_js(ctx),
            AttributeValue::Number(n) => n.into_js(ctx),
            AttributeValue::Bool(b) => b.into_js(ctx),
        })
        .transpose()?;
    or_null(ctx, value)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::{callback_ids, engine_with_bindings};
    use inca_gpui::NodeId;
    use rstest::rstest;

    #[test]
    fn root_node_id_returns_the_host_root() {
        let (engine, host) = engine_with_bindings();

        let root_id: NodeId = engine.eval("__inca_native__.rootNodeId();").unwrap();

        assert_eq!(root_id, host.borrow().root());
        assert_eq!(host.borrow().tree.get(root_id).unwrap().tag_name(), "div");
    }

    #[test]
    fn root_node_id_is_stable_across_calls_and_tree_growth() {
        let (engine, _host) = engine_with_bindings();

        let same: bool = engine
            .eval(
                r"
                const first = __inca_native__.rootNodeId();
                __inca_native__.createNode('div');
                first === __inca_native__.rootNodeId();
                ",
            )
            .unwrap();

        assert!(
            same,
            "the root handle must outlive any node created after it"
        );
    }

    #[test]
    fn get_attribute_returns_the_stored_value_in_its_own_type() {
        let (engine, _host) = engine_with_bindings();

        let same: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'label', 'hello');
                n.setAttribute(node, 'count', 3);
                n.setAttribute(node, 'visible', true);
                n.getAttribute(node, 'label') === 'hello' &&
                    n.getAttribute(node, 'count') === 3 &&
                    n.getAttribute(node, 'visible') === true;
                ",
            )
            .unwrap();

        assert!(same, "string, number and boolean must read back unchanged");
    }

    #[test]
    fn an_attribute_name_is_case_insensitive_on_set_and_get() {
        let (engine, _host) = engine_with_bindings();

        let same: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'ID', 'x');
                n.getAttribute(node, 'id') === 'x' && n.getAttribute(node, 'Id') === 'x';
                ",
            )
            .unwrap();

        assert!(same);
    }

    #[test]
    fn remove_attribute_folds_the_name_to_ascii_lower_case() {
        let (engine, _host) = engine_with_bindings();

        let gone: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'id', 'x');
                n.removeAttribute(node, 'ID');
                n.getAttribute(node, 'id') === null;
                ",
            )
            .unwrap();

        assert!(gone);
    }

    #[test]
    fn attribute_names_returns_the_sorted_keys() {
        let (engine, _host) = engine_with_bindings();

        let names: Vec<String> = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'id', 'x');
                n.setAttribute(node, 'Title', 't');
                n.setAttribute(node, 'a', 1);
                n.attributeNames(node);
                ",
            )
            .unwrap();

        assert_eq!(names, ["a", "id", "title"]);
    }

    #[test]
    fn attribute_names_of_a_node_without_attributes_is_empty() {
        let (engine, _host) = engine_with_bindings();

        let names: Vec<String> = engine
            .eval("__inca_native__.attributeNames(__inca_native__.createNode('div'));")
            .unwrap();

        assert!(names.is_empty());
    }

    #[test]
    fn attribute_names_on_an_unknown_node_throws_type_error() {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            crate::test_support::error_name(&engine, "__inca_native__.attributeNames(999)"),
            "TypeError"
        );
    }

    #[test]
    fn get_attribute_of_a_missing_key_is_null() {
        let (engine, _host) = engine_with_bindings();

        let is_null: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.getAttribute(node, 'never_set') === null;
                ",
            )
            .unwrap();

        assert!(is_null);
    }

    #[test]
    fn get_attribute_on_an_unknown_node_throws_type_error() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.getAttribute(999, 'label');
                } catch (e) {
                    caught = e instanceof TypeError;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught, "an unknown node id must raise a TypeError");
    }

    #[rstest]
    #[case::nan("NaN")]
    #[case::fractional("1.5")]
    #[case::negative("-1")]
    #[case::infinity("Infinity")]
    #[case::above_u32("2 ** 32")]
    #[case::numeric_string("'1'")]
    fn node_id_that_is_not_a_whole_u32_throws_type_error(#[case] bad: &str) {
        let (engine, _host) = engine_with_bindings();

        let script = format!(
            r"
            (() => {{ const node = __inca_native__.createNode('div');
            const rejects = (f) => {{
                try {{ f(); return false; }} catch (e) {{ return e instanceof TypeError; }}
            }};
            return [
                rejects(() => __inca_native__.setStyle({bad}, 'width', 1)),
                rejects(() => __inca_native__.removeStyle({bad}, 'width')),
                rejects(() => __inca_native__.getAttribute({bad}, 'label')),
                rejects(() => __inca_native__.removeAttribute({bad}, 'label')),
                rejects(() => __inca_native__.appendChild({bad}, node)),
                rejects(() => __inca_native__.appendChild(node, {bad})),
                rejects(() => __inca_native__.insertBefore(node, node, {bad})),
                rejects(() => __inca_native__.destroyNode({bad})),
                rejects(() => __inca_native__.holdNode({bad})),
                rejects(() => __inca_native__.releaseNode({bad})),
                rejects(() => __inca_native__.addEventListener({bad}, 'click')),
                rejects(() => __inca_native__.removeEventListener({bad}, 'click', 1)),
                rejects(() => __inca_native__.focusNode({bad})),
                rejects(() => __inca_native__.dispatchEvent({bad}, {{}})),
                rejects(() => __inca_native__.click({bad})),
            ].every(Boolean); }})()
            "
        );
        assert!(engine.eval::<bool>(&script).unwrap(), "{bad}");
    }

    #[test]
    fn a_normal_node_id_and_an_absent_anchor_still_work() {
        let (engine, _host) = engine_with_bindings();

        let ok: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const p = n.createNode('div');
                const c = n.createNode('div');
                n.setStyle(c, 'width', 1);
                n.insertBefore(p, c, undefined);
                n.destroyNode(c);
                true;
                ",
            )
            .unwrap();

        assert!(ok);
    }

    #[test]
    fn unknown_node_id_throws_catchable_exception() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.appendChild(999, 1000);
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(
            caught,
            "an unknown node id must raise a catchable exception"
        );
    }

    #[test]
    fn non_primitive_attribute_value_throws_catchable_exception() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                let caught = false;
                try {
                    __inca_native__.setAttribute(node, 'bad', { nested: true });
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(
            caught,
            "a non-primitive attribute value must raise a catchable exception"
        );
    }

    #[test]
    fn remove_attribute_on_an_unknown_node_throws() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                let caught = false;
                try {
                    n.removeAttribute(999, 'x');
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught);
    }

    #[test]
    fn insert_before_unknown_ids_raise_catchable_exception() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                const parent = __inca_native__.createNode('div');
                const child = __inca_native__.createNode('span');
                let caught = false;
                try {
                    __inca_native__.insertBefore(parent, child, 999);
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(
            caught,
            "an unknown anchor id must raise a catchable exception"
        );
    }

    #[test]
    fn hold_node_throws_type_error_for_an_unknown_id() {
        let (engine, _host) = engine_with_bindings();

        let type_error: bool = engine
            .eval(
                r"
                let type_error = false;
                try {
                    __inca_native__.holdNode(999);
                } catch (e) {
                    type_error = e instanceof TypeError;
                }
                type_error;
                ",
            )
            .unwrap();

        assert!(type_error);
    }

    #[test]
    fn release_node_ignores_an_unknown_id() {
        let (engine, _host) = engine_with_bindings();

        let threw: bool = engine
            .eval(
                r"
                let threw = false;
                try {
                    __inca_native__.releaseNode(999);
                } catch (e) {
                    threw = true;
                }
                threw;
                ",
            )
            .unwrap();

        assert!(!threw);
    }

    #[test]
    fn a_held_node_survives_a_sweep_until_it_is_released() {
        let (engine, host) = engine_with_bindings();
        let node: NodeId = engine
            .eval("const n = __inca_native__.createNode('div'); __inca_native__.holdNode(n); n;")
            .unwrap();

        assert!(host.borrow_mut().sweep_orphans().is_none());
        assert!(host.borrow().tree.get(node).is_some());

        engine
            .eval::<()>(&format!("__inca_native__.releaseNode({node})"))
            .unwrap();
        let (nodes, _) = host.borrow_mut().sweep_orphans().unwrap();
        assert_eq!(nodes, [node]);
    }

    #[test]
    fn sweep_orphans_releases_the_listeners_of_the_swept_subtree() {
        let (engine, host) = engine_with_bindings();
        let ids: Vec<u32> = engine
            .eval(
                r"
                const n = __inca_native__;
                const root = n.rootNodeId();
                const kept = n.createNode('div');
                n.appendChild(root, kept);
                n.addEventListener(kept, 'click');
                const branch = n.createNode('div');
                const leaf = n.createNode('div');
                n.appendChild(branch, leaf);
                n.addEventListener(branch, 'click');
                n.addEventListener(leaf, 'click');
                [kept, branch, leaf];
                ",
            )
            .unwrap();
        let (kept, branch, leaf) = (ids[0], ids[1], ids[2]);

        let (mut nodes, mut callbacks) = host.borrow_mut().sweep_orphans().unwrap();
        nodes.sort_unstable();
        callbacks.sort_unstable();

        assert_eq!(nodes, [branch, leaf]);
        assert_eq!(callbacks, [1, 2]);
        let host = host.borrow();
        assert!(
            callback_ids(&host.listeners, crate::EventTargetId::Node(branch), "click").is_empty()
        );
        assert!(
            callback_ids(&host.listeners, crate::EventTargetId::Node(leaf), "click").is_empty()
        );
        assert_eq!(
            callback_ids(&host.listeners, crate::EventTargetId::Node(kept), "click"),
            [0]
        );
    }

    #[test]
    fn sweep_orphans_forgets_the_queued_focus_work_of_a_swept_node() {
        let (engine, host) = engine_with_bindings();
        let node: NodeId = engine.eval("__inca_native__.createNode('div')").unwrap();
        host.borrow_mut().focus.mark_tab_dirty(node);

        host.borrow_mut().sweep_orphans().unwrap();

        assert!(host.borrow_mut().focus.take_tab_dirty().is_empty());
    }
}
