// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, IntoJs, Result as JsResult, Value};

use inca_gpui::{AttributeValue, NodeId, VirtualNode, style_warning};

use super::Natives;
use super::convert::{JsNodeId, existing_node, or_null, throw_tree_error};
use super::host::Host;

/// The retained tree itself: allocating, attaching, and describing nodes,
/// plus `destroyNode`, `holdNode` and `releaseNode`. Freeing a subtree is a
/// tree operation first, even though it also releases the listener/focus
/// state that named its nodes.
pub(super) fn install_tree(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("rootNodeId", root_node_id)?;
    n.set("createNode", create_node)?;
    n.set("appendChild", append_child)?;
    n.set("insertBefore", insert_before)?;
    n.set("removeChild", remove_child)?;
    n.set("setAttribute", set_attribute)?;
    n.set("getAttribute", get_attribute)?;
    n.set("removeAttribute", remove_attribute)?;
    n.set("setStyle", set_style)?;
    n.set("removeStyle", remove_style)?;
    n.set("destroyNode", destroy_node)?;
    n.set("holdNode", hold_node)?;
    n.set("releaseNode", release_node)
}

fn root_node_id(host: Rc<RefCell<Host>>) -> impl Fn() -> NodeId {
    move || host.borrow().root()
}

fn create_node<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, String) -> JsResult<NodeId> {
    move |ctx, tag_name| {
        let mut host = host.borrow_mut();
        host.tree
            .create_node(tag_name)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn append_child<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<()> {
    move |ctx, JsNodeId(parent_id), JsNodeId(child_id)| {
        let mut host = host.borrow_mut();
        refuse_root(&ctx, &host, child_id, "moved")?;
        let attached = is_attached(&host, child_id);
        host.tree
            .append_child(parent_id, child_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        if attached {
            drop_focus(&mut host, child_id);
        }
        mark_focus_dirty(&mut host, child_id);
        Ok(())
    }
}

fn insert_before<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId, Option<JsNodeId>) -> JsResult<()> {
    move |ctx, JsNodeId(parent_id), JsNodeId(child_id), anchor_id| {
        let mut host = host.borrow_mut();
        refuse_root(&ctx, &host, child_id, "moved")?;
        let attached = is_attached(&host, child_id);
        host.tree
            .insert_before(parent_id, child_id, anchor_id.map(|JsNodeId(id)| id))
            .map_err(|err| throw_tree_error(&ctx, err))?;
        if attached {
            drop_focus(&mut host, child_id);
        }
        mark_focus_dirty(&mut host, child_id);
        Ok(())
    }
}

fn remove_child<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<()> {
    move |ctx, JsNodeId(parent_id), JsNodeId(child_id)| {
        let mut host = host.borrow_mut();
        let child_of_parent = host
            .tree
            .get(child_id)
            .is_some_and(|node| node.parent() == Some(parent_id));
        host.tree
            .remove_child(parent_id, child_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        if child_of_parent {
            drop_focus(&mut host, child_id);
            mark_focus_dirty(&mut host, child_id);
        }
        Ok(())
    }
}

fn set_attribute<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, Value<'js>) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key, value| {
        let value = attribute_value_from_js(&ctx, &value)?;
        let mut host = host.borrow_mut();
        let key = canonical_attribute_key(key);
        host.tree
            .set_attribute(node_id, key.clone(), value)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        mark_focus_dirty_for(&mut host, node_id, &key);
        Ok(())
    }
}

fn get_attribute<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), key| {
        let key = canonical_attribute_key(key);
        let host = host.borrow();
        let node = existing_node(&ctx, &host.tree, node_id)?;
        attribute_value_to_js(&ctx, node.attributes().get(&key))
    }
}

fn remove_attribute<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key| {
        let mut host = host.borrow_mut();
        let key = canonical_attribute_key(key);
        host.tree
            .remove_attribute(node_id, &key)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        mark_focus_dirty_for(&mut host, node_id, &key);
        Ok(())
    }
}

fn set_style<'js>(
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
        if let Some(warning) = warning {
            let tag = host.tree.get(node_id).map_or("", VirtualNode::tag_name);
            (host.warn)(&format!("node {node_id} ({tag}): {warning}"));
        }
        Ok(())
    }
}

fn remove_style<'js>(
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

fn destroy_node<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Vec<u32>> {
    move |ctx, JsNodeId(node_id)| {
        let mut host = host.borrow_mut();
        refuse_root(&ctx, &host, node_id, "destroyed")?;
        let freed = host.tree.destroy_node(node_id);
        Ok(host.free_ids(&freed))
    }
}

fn hold_node<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<()> {
    move |ctx, JsNodeId(node_id)| {
        host.borrow_mut()
            .tree
            .hold(node_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn release_node(host: Rc<RefCell<Host>>) -> impl Fn(JsNodeId) {
    move |JsNodeId(node_id)| host.borrow_mut().tree.release(node_id)
}

/// Refuses `id` when it is the root node, which belongs to the host. `verb`
/// completes the message, as in "cannot be {verb}".
///
/// # Errors
///
/// Throws a `TypeError` when `id` is the root.
fn refuse_root(ctx: &Ctx<'_>, host: &Host, id: NodeId, verb: &str) -> JsResult<()> {
    if id == host.root() {
        return Err(Exception::throw_type(
            ctx,
            &format!("the root node belongs to the host and cannot be {verb}"),
        ));
    }
    Ok(())
}

fn is_attached(host: &Host, node_id: NodeId) -> bool {
    host.tree
        .get(node_id)
        .is_some_and(|node| node.parent().is_some())
}

/// Called when `removeChild` detaches a node or an insert moves an attached
/// one. A node that was never attached holds no focus to drop.
fn drop_focus(host: &mut Host, node_id: NodeId) {
    host.focus.drop_focus_in(&host.tree, node_id);
}

/// Queues a focus re-check of the node and its subtree.
fn mark_focus_dirty(host: &mut Host, node_id: NodeId) {
    host.focus.mark_subtree_dirty(&host.tree, node_id);
}

/// Queues the focus re-check an attribute change needs.
fn mark_focus_dirty_for(host: &mut Host, node_id: NodeId, key: &str) {
    match key {
        "tabindex" | "disabled" => host.focus.mark_tab_dirty(node_id),
        "hidden" | "inert" => mark_focus_dirty(host, node_id),
        _ => {}
    }
}

/// Attribute names are case-insensitive, so `tabIndex` is `tabindex`.
fn canonical_attribute_key(key: String) -> String {
    if key.eq_ignore_ascii_case("tabindex") {
        "tabindex".to_owned()
    } else {
        key
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
mod tests;
