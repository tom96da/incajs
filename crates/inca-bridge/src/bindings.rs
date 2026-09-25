// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds `globalThis.__inca_native__`: the small set of native functions
//! JS calls to read the root handle, mutate the retained virtual tree,
//! register input-event callbacks, and free what it no longer needs.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, Function, Object, Result as JsResult, Value};

use inca_gpui::{AttributeValue, NodeId, TreeError, VirtualTree};

use crate::focus::FocusRegistry;

/// Every `(node id, event name)` a JS caller has registered through
/// `addEventListener`, mapped to the plain integer callback handles it gave
/// us. Only ever stores owned Rust data — never a JS value or function —
/// so it stays readable by Rust code running outside of any JS call, such
/// as the input-event dispatcher.
///
/// Nothing expires on its own: a registration lives until
/// [`unregister`](Self::unregister) drops it, or [`release`](Self::release)
/// does when the node goes away.
#[derive(Debug, Default)]
pub struct EventListeners {
    by_node: HashMap<NodeId, HashMap<String, Vec<u32>>>,
}

impl EventListeners {
    /// Adds `callback_id` to `(node_id, event)`. Registering an id already
    /// there is a no-op, not a second entry.
    pub fn register(&mut self, node_id: NodeId, event: impl Into<String>, callback_id: u32) {
        let callbacks = self
            .by_node
            .entry(node_id)
            .or_default()
            .entry(event.into())
            .or_default();
        if !callbacks.contains(&callback_id) {
            callbacks.push(callback_id);
        }
    }

    /// Drops `callback_id` from `(node_id, event)`, and reports whether it
    /// was registered.
    pub fn unregister(&mut self, node_id: NodeId, event: &str, callback_id: u32) -> bool {
        let Some(events) = self.by_node.get_mut(&node_id) else {
            return false;
        };
        let Some(callbacks) = events.get_mut(event) else {
            return false;
        };
        let before = callbacks.len();
        callbacks.retain(|&id| id != callback_id);
        let removed = callbacks.len() != before;
        if callbacks.is_empty() {
            events.remove(event);
        }
        if events.is_empty() {
            self.by_node.remove(&node_id);
        }
        removed
    }

    /// Drops every registration on `node_id` and returns the callback ids
    /// that were there, in no particular order — the JS side needs them to
    /// drop the functions they name.
    pub fn release(&mut self, node_id: NodeId) -> Vec<u32> {
        self.by_node
            .remove(&node_id)
            .map(|events| events.into_values().flatten().collect())
            .unwrap_or_default()
    }

    pub fn callbacks_for(&self, node_id: NodeId, event: &str) -> &[u32] {
        self.by_node
            .get(&node_id)
            .and_then(|events| events.get(event))
            .map_or(&[], Vec::as_slice)
    }
}

/// Everything one `__inca_native__` binding set shares: the retained tree
/// it mutates, the root a mounting app attaches under, and the
/// event-listener registrations it records.
#[derive(Debug)]
pub struct Host {
    pub tree: VirtualTree,
    /// Allocated with the tree, so `rootNodeId` always resolves.
    pub root: NodeId,
    pub listeners: EventListeners,
    pub focus: FocusRegistry,
}

impl Default for Host {
    fn default() -> Self {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div");
        Self {
            tree,
            root,
            listeners: EventListeners::default(),
            focus: FocusRegistry::default(),
        }
    }
}

fn throw_tree_error(ctx: &Ctx<'_>, err: TreeError) -> rquickjs::Error {
    Exception::throw_type(ctx, &err.to_string())
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

/// Installs `globalThis.__inca_native__` into `ctx`, wired to `host`.
///
/// # Errors
///
/// Returns an error if defining `globalThis.__inca_native__` or any of its
/// methods on `ctx` fails.
// Long from repeating one registration block per binding, not from
// complexity — splitting it up would just spread that same list across more
// functions.
#[allow(clippy::too_many_lines)]
pub fn install<'js>(ctx: &Ctx<'js>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
    let native = Object::new(ctx.clone())?;

    {
        let host = Rc::clone(host);
        native.set(
            "rootNodeId",
            Function::new(ctx.clone(), move || -> NodeId { host.borrow().root })?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "createNode",
            Function::new(ctx.clone(), move |tag_name: String| -> NodeId {
                host.borrow_mut().tree.create_node(tag_name)
            })?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "appendChild",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, parent_id: NodeId, child_id: NodeId| -> JsResult<()> {
                    host.borrow_mut()
                        .tree
                        .append_child(parent_id, child_id)
                        .map_err(|err| throw_tree_error(&ctx, err))
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "insertBefore",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      parent_id: NodeId,
                      child_id: NodeId,
                      anchor_id: Option<NodeId>|
                      -> JsResult<()> {
                    host.borrow_mut()
                        .tree
                        .insert_before(parent_id, child_id, anchor_id)
                        .map_err(|err| throw_tree_error(&ctx, err))
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "removeChild",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, parent_id: NodeId, child_id: NodeId| -> JsResult<()> {
                    host.borrow_mut()
                        .tree
                        .remove_child(parent_id, child_id)
                        .map_err(|err| throw_tree_error(&ctx, err))
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "setAttribute",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      node_id: NodeId,
                      key: String,
                      value: Value<'js>|
                      -> JsResult<()> {
                    let value = attribute_value_from_js(&ctx, &value)?;
                    host.borrow_mut()
                        .tree
                        .set_attribute(node_id, key, value)
                        .map_err(|err| throw_tree_error(&ctx, err))
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "setStyle",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      node_id: NodeId,
                      key: String,
                      value: Value<'js>|
                      -> JsResult<()> {
                    let value = attribute_value_from_js(&ctx, &value)?;
                    host.borrow_mut()
                        .tree
                        .set_style(node_id, key, value)
                        .map_err(|err| throw_tree_error(&ctx, err))
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "addEventListener",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      node_id: NodeId,
                      event: String,
                      callback_id: u32|
                      -> JsResult<()> {
                    let mut host = host.borrow_mut();
                    if host.tree.get(node_id).is_none() {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    }
                    host.listeners.register(node_id, event, callback_id);
                    Ok(())
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "removeEventListener",
            Function::new(
                ctx.clone(),
                move |node_id: NodeId, event: String, callback_id: u32| -> bool {
                    host.borrow_mut()
                        .listeners
                        .unregister(node_id, &event, callback_id)
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "destroyNode",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, node_id: NodeId| -> JsResult<Vec<u32>> {
                    let mut host = host.borrow_mut();
                    if node_id == host.root {
                        return Err(Exception::throw_type(
                            &ctx,
                            "the root node belongs to the host and cannot be destroyed",
                        ));
                    }
                    let freed = host.tree.destroy_node(node_id);
                    Ok(freed
                        .into_iter()
                        .flat_map(|id| {
                            host.focus.forget(id);
                            host.listeners.release(id)
                        })
                        .collect())
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "focusNode",
            Function::new(ctx.clone(), move |node_id: NodeId| {
                host.borrow_mut().focus.request_focus(node_id);
            })?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "blurNode",
            Function::new(ctx.clone(), move || {
                host.borrow_mut().focus.request_blur();
            })?,
        )?;
    }

    ctx.globals().set("__inca_native__", native)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use inca_jsenv::Engine;

    fn engine_with_bindings() -> (Engine, Rc<RefCell<Host>>) {
        let engine = Engine::new().unwrap();
        let host = Rc::new(RefCell::new(Host::default()));
        engine.with(|ctx| install(&ctx, &host)).unwrap();
        (engine, host)
    }

    #[test]
    fn registering_the_same_callback_twice_keeps_one_entry() {
        let mut listeners = EventListeners::default();
        listeners.register(1, "click", 7);
        listeners.register(1, "click", 7);

        assert_eq!(listeners.callbacks_for(1, "click"), &[7]);
    }

    #[test]
    fn distinct_callbacks_on_one_event_stack() {
        let mut listeners = EventListeners::default();
        listeners.register(1, "click", 7);
        listeners.register(1, "click", 8);

        assert_eq!(listeners.callbacks_for(1, "click"), &[7, 8]);
    }

    #[test]
    fn unregister_reports_whether_it_removed_anything() {
        let mut listeners = EventListeners::default();
        listeners.register(1, "click", 7);

        assert!(listeners.unregister(1, "click", 7));
        assert!(listeners.callbacks_for(1, "click").is_empty());
        assert!(!listeners.unregister(1, "click", 7));
        assert!(!listeners.unregister(99, "click", 7));
    }

    #[test]
    fn release_returns_every_callback_on_a_node() {
        let mut listeners = EventListeners::default();
        listeners.register(1, "click", 7);
        listeners.register(1, "focus", 8);
        listeners.register(2, "click", 9);

        let mut released = listeners.release(1);
        released.sort_unstable();

        assert_eq!(released, vec![7, 8]);
        assert!(listeners.callbacks_for(1, "click").is_empty());
        assert_eq!(
            listeners.callbacks_for(2, "click"),
            &[9],
            "another node's registrations must survive"
        );
    }

    #[test]
    fn destroy_node_frees_the_subtree_and_hands_back_its_callback_ids() {
        let (engine, host) = engine_with_bindings();

        let released: Vec<u32> = engine
            .eval(
                r"
                const root = __inca_native__.rootNodeId();
                const branch = __inca_native__.createNode('div');
                const leaf = __inca_native__.createNode('text');
                __inca_native__.appendChild(root, branch);
                __inca_native__.appendChild(branch, leaf);
                __inca_native__.addEventListener(branch, 'click', 1);
                __inca_native__.addEventListener(leaf, 'click', 2);

                __inca_native__.destroyNode(branch);
                ",
            )
            .unwrap();
        let mut released = released;
        released.sort_unstable();

        assert_eq!(
            released,
            vec![1, 2],
            "a descendant's callback is unreachable from JS once its root is gone"
        );

        let host = host.borrow();
        assert!(host.tree.get(host.root).unwrap().children().is_empty());
        assert!(host.listeners.callbacks_for(1, "click").is_empty());
    }

    #[test]
    fn destroy_node_is_idempotent() {
        let (engine, _host) = engine_with_bindings();

        let second_pass: Vec<u32> = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                __inca_native__.destroyNode(node);
                __inca_native__.destroyNode(node);
                ",
            )
            .unwrap();

        assert!(second_pass.is_empty());
    }

    #[test]
    fn destroying_the_root_raises_a_catchable_exception() {
        let (engine, host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.destroyNode(__inca_native__.rootNodeId());
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught, "the root belongs to the host, not to the app");
        let host = host.borrow();
        assert!(host.tree.get(host.root).is_some());
    }

    #[test]
    fn remove_event_listener_drops_the_registration() {
        let (engine, host) = engine_with_bindings();

        let removed: bool = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                __inca_native__.addEventListener(node, 'click', 3);
                __inca_native__.removeEventListener(node, 'click', 3);
                ",
            )
            .unwrap();

        assert!(removed);
        assert!(host.borrow().listeners.callbacks_for(1, "click").is_empty());
    }

    #[test]
    fn remove_event_listener_on_a_gone_node_is_not_an_error() {
        let (engine, _host) = engine_with_bindings();

        let removed: bool = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                __inca_native__.destroyNode(node);
                __inca_native__.removeEventListener(node, 'click', 3);
                ",
            )
            .unwrap();

        assert!(
            !removed,
            "removing after a destroy races normal teardown and must not throw"
        );
    }

    #[test]
    fn root_node_id_returns_the_host_root() {
        let (engine, host) = engine_with_bindings();

        let root_id: NodeId = engine.eval("__inca_native__.rootNodeId();").unwrap();

        assert_eq!(root_id, host.borrow().root);
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
    fn happy_path_round_trip_through_eval() {
        let (engine, host) = engine_with_bindings();

        let ids: Vec<NodeId> = engine
            .eval(
                r"
                const parent = __inca_native__.createNode('div');
                const child = __inca_native__.createNode('span');
                __inca_native__.appendChild(parent, child);

                const stray = __inca_native__.createNode('span');
                __inca_native__.appendChild(parent, stray);
                __inca_native__.removeChild(parent, stray);

                const first = __inca_native__.createNode('span');
                __inca_native__.insertBefore(parent, first, child);

                __inca_native__.setAttribute(child, 'label', 'hello');
                __inca_native__.setAttribute(child, 'count', 3);
                __inca_native__.setAttribute(child, 'visible', true);
                __inca_native__.setStyle(child, 'display', 'flex');
                __inca_native__.addEventListener(child, 'click', 7);

                [parent, child];
                ",
            )
            .unwrap();
        let [parent_id, child_id] = ids[..] else {
            panic!("expected exactly the parent and child ids");
        };

        let host = host.borrow();
        let parent = host.tree.get(parent_id).unwrap();
        assert_eq!(parent.tag_name(), "div");
        assert_eq!(
            parent.children().len(),
            2,
            "the removed stray child must not remain attached"
        );
        assert_eq!(
            parent.children()[1],
            child_id,
            "insertBefore(parent, first, child) must place `first` ahead of `child`"
        );

        let child = host.tree.get(child_id).unwrap();
        assert_eq!(child.tag_name(), "span");
        assert_eq!(
            child.attributes().get("label"),
            Some(&AttributeValue::String("hello".into()))
        );
        assert_eq!(
            child.attributes().get("count"),
            Some(&AttributeValue::Number(3.0))
        );
        assert_eq!(
            child.attributes().get("visible"),
            Some(&AttributeValue::Bool(true))
        );
        assert_eq!(
            child.style_props().get("display"),
            Some(&AttributeValue::String("flex".into()))
        );
        assert_eq!(host.listeners.callbacks_for(child_id, "click"), &[7]);
    }

    #[test]
    fn unknown_node_id_raises_catchable_exception() {
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
    fn non_primitive_attribute_value_raises_catchable_exception() {
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
    fn set_style_unknown_node_id_raises_catchable_exception() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.setStyle(999, 'display', 'flex');
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
    fn set_style_non_primitive_value_raises_catchable_exception() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                let caught = false;
                try {
                    __inca_native__.setStyle(node, 'bad', { nested: true });
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(
            caught,
            "a non-primitive style value must raise a catchable exception"
        );
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
}
