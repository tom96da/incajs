// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds `globalThis.__inca_native__`: the small set of native functions
//! JS calls to read the root handle, mutate the retained virtual tree,
//! register input-event callbacks, and free what it no longer needs.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, FromJs, Function, Object, Result as JsResult, Value};

use inca_gpui::{AttributeValue, NodeId, TreeError, VirtualNode, VirtualTree, style_warning};

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
pub struct Host {
    pub tree: VirtualTree,
    /// Receives one line for each style a `setStyle` call stores with an
    /// unknown key or an invalid color. Writes to stderr by default.
    pub warn: Rc<dyn Fn(&str)>,
    /// Allocated with the tree, so `rootNodeId` always resolves.
    pub root: NodeId,
    pub listeners: EventListeners,
    pub focus: FocusRegistry,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("tree", &self.tree)
            .field("root", &self.root)
            .field("listeners", &self.listeners)
            .field("focus", &self.focus)
            .finish_non_exhaustive()
    }
}

impl Default for Host {
    fn default() -> Self {
        let mut tree = VirtualTree::new();
        let root = tree
            .create_node("div")
            .expect("a new tree has free node ids");
        Self {
            tree,
            warn: Rc::new(|line| eprintln!("{line}")),
            root,
            listeners: EventListeners::default(),
            focus: FocusRegistry::default(),
        }
    }
}

/// A node id argument. Accepts a finite whole number in `u32` range;
/// anything else (`NaN`, `1.5`, `-1`, `Infinity`, `2**32`, a non-number)
/// throws a `TypeError`.
#[derive(Debug, Clone, Copy)]
struct JsNodeId(NodeId);

impl<'js> FromJs<'js> for JsNodeId {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        let number = value
            .as_number()
            .ok_or_else(|| Exception::throw_type(ctx, "node id must be a number"))?;
        if number.fract() == 0.0 && (0.0..=f64::from(NodeId::MAX)).contains(&number) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            return Ok(Self(number as NodeId));
        }
        Err(Exception::throw_type(
            ctx,
            "node id must be a whole number in the range of an unsigned 32-bit integer",
        ))
    }
}

/// Refuses `id` when it is the root node, which belongs to the host. `verb`
/// completes the message, as in "cannot be {verb}".
///
/// # Errors
///
/// Throws a `TypeError` when `id` is the root.
fn refuse_root(ctx: &Ctx<'_>, host: &Host, id: NodeId, verb: &str) -> JsResult<()> {
    if id == host.root {
        return Err(Exception::throw_type(
            ctx,
            &format!("the root node belongs to the host and cannot be {verb}"),
        ));
    }
    Ok(())
}

fn throw_tree_error(ctx: &Ctx<'_>, err: TreeError) -> rquickjs::Error {
    Exception::throw_type(ctx, &err.to_string())
}

// Queues a focus re-check of the node and its subtree.
fn mark_focus_dirty(host: &mut Host, node_id: NodeId) {
    host.focus.mark_subtree_dirty(&host.tree, node_id);
}

// Queues the focus re-check an attribute change needs.
fn mark_focus_dirty_for(host: &mut Host, node_id: NodeId, key: &str) {
    match key {
        "tabindex" | "disabled" => host.focus.mark_tab_dirty(node_id),
        "hidden" | "inert" => mark_focus_dirty(host, node_id),
        _ => {}
    }
}

// Attribute names are case-insensitive, so `tabIndex` is `tabindex`.
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

/// The retained tree itself: allocating, attaching, and describing nodes,
/// plus `destroyNode` — freeing a subtree is a tree operation first, even
/// though it also releases the listener/focus state that named its nodes.
// Long from repeating one registration block per binding, not from
// complexity — splitting it up further would just spread that same list
// across more functions.
#[allow(clippy::too_many_lines)]
fn install_tree<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    native: &Object<'js>,
) -> JsResult<()> {
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
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, tag_name: String| -> JsResult<NodeId> {
                    let mut host = host.borrow_mut();
                    host.tree
                        .create_node(tag_name)
                        .map_err(|err| throw_tree_error(&ctx, err))
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "appendChild",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      JsNodeId(parent_id): JsNodeId,
                      JsNodeId(child_id): JsNodeId|
                      -> JsResult<()> {
                    let mut host = host.borrow_mut();
                    refuse_root(&ctx, &host, child_id, "moved")?;
                    host.tree
                        .append_child(parent_id, child_id)
                        .map_err(|err| throw_tree_error(&ctx, err))?;
                    mark_focus_dirty(&mut host, child_id);
                    Ok(())
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
                      JsNodeId(parent_id): JsNodeId,
                      JsNodeId(child_id): JsNodeId,
                      anchor_id: Option<JsNodeId>|
                      -> JsResult<()> {
                    let mut host = host.borrow_mut();
                    refuse_root(&ctx, &host, child_id, "moved")?;
                    host.tree
                        .insert_before(parent_id, child_id, anchor_id.map(|JsNodeId(id)| id))
                        .map_err(|err| throw_tree_error(&ctx, err))?;
                    mark_focus_dirty(&mut host, child_id);
                    Ok(())
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
                move |ctx: Ctx<'js>,
                      JsNodeId(parent_id): JsNodeId,
                      JsNodeId(child_id): JsNodeId|
                      -> JsResult<()> {
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
                      JsNodeId(node_id): JsNodeId,
                      key: String,
                      value: Value<'js>|
                      -> JsResult<()> {
                    let value = attribute_value_from_js(&ctx, &value)?;
                    let mut host = host.borrow_mut();
                    let key = canonical_attribute_key(key);
                    host.tree
                        .set_attribute(node_id, key.clone(), value)
                        .map_err(|err| throw_tree_error(&ctx, err))?;
                    mark_focus_dirty_for(&mut host, node_id, &key);
                    Ok(())
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "removeAttribute",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, JsNodeId(node_id): JsNodeId, key: String| -> JsResult<()> {
                    let mut host = host.borrow_mut();
                    let key = canonical_attribute_key(key);
                    host.tree
                        .remove_attribute(node_id, &key)
                        .map_err(|err| throw_tree_error(&ctx, err))?;
                    mark_focus_dirty_for(&mut host, node_id, &key);
                    Ok(())
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
                      JsNodeId(node_id): JsNodeId,
                      key: String,
                      value: Value<'js>|
                      -> JsResult<()> {
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
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "removeStyle",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, JsNodeId(node_id): JsNodeId, key: String| -> JsResult<()> {
                    let mut host = host.borrow_mut();
                    host.tree
                        .remove_style(node_id, &key)
                        .map_err(|err| throw_tree_error(&ctx, err))?;
                    if key == "display" {
                        mark_focus_dirty(&mut host, node_id);
                    }
                    Ok(())
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
                move |ctx: Ctx<'js>, JsNodeId(node_id): JsNodeId| -> JsResult<Vec<u32>> {
                    let mut host = host.borrow_mut();
                    refuse_root(&ctx, &host, node_id, "destroyed")?;
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

    Ok(())
}

/// Registering and dropping input-event callbacks.
fn install_events<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    native: &Object<'js>,
) -> JsResult<()> {
    {
        let host = Rc::clone(host);
        native.set(
            "addEventListener",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      JsNodeId(node_id): JsNodeId,
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
                move |JsNodeId(node_id): JsNodeId, event: String, callback_id: u32| -> bool {
                    host.borrow_mut()
                        .listeners
                        .unregister(node_id, &event, callback_id)
                },
            )?,
        )?;
    }

    Ok(())
}

/// Requesting and releasing focus.
fn install_focus<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    native: &Object<'js>,
) -> JsResult<()> {
    {
        let host = Rc::clone(host);
        native.set(
            "focusNode",
            Function::new(ctx.clone(), move |JsNodeId(node_id): JsNodeId| {
                let mut host = host.borrow_mut();
                // Focusing a node that is gone is a no-op, as in the DOM.
                if host.tree.get(node_id).is_some() {
                    host.focus.request_focus(node_id);
                }
            })?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "blurNode",
            Function::new(ctx.clone(), move |JsNodeId(node_id): JsNodeId| {
                host.borrow_mut().focus.request_blur(node_id);
            })?,
        )?;
    }

    Ok(())
}

/// Installs `globalThis.__inca_native__` into `ctx`, wired to `host`.
///
/// # Errors
///
/// Returns an error if defining `globalThis.__inca_native__` or any of its
/// methods on `ctx` fails.
pub fn install(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
    let native = Object::new(ctx.clone())?;
    install_tree(ctx, host, &native)?;
    install_events(ctx, host, &native)?;
    install_focus(ctx, host, &native)?;
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
    fn only_focus_attributes_and_display_mark_a_node_for_a_focus_check() {
        let (engine, host) = engine_with_bindings();
        let node: u32 = engine
            .eval("globalThis.n = __inca_native__.createNode('div')")
            .unwrap();
        let dirty =
            |host: &Rc<RefCell<Host>>| std::mem::take(&mut host.borrow_mut().focus.tab_dirty);
        assert!(dirty(&host).is_empty());

        engine
            .eval::<()>(
                "__inca_native__.setAttribute(n, 'label', 'x'); \
                 __inca_native__.removeAttribute(n, 'label'); \
                 __inca_native__.setStyle(n, 'width', 5); \
                 __inca_native__.removeStyle(n, 'width');",
            )
            .unwrap();
        assert!(dirty(&host).is_empty());

        // A subtree check queues only nodes that have a tabindex.
        for call in [
            "setAttribute(n, 'tabindex', '0')",
            "setAttribute(n, 'tabIndex', 0)",
            "setAttribute(n, 'disabled', true)",
            "removeAttribute(n, 'disabled')",
            "setStyle(n, 'display', 'none')",
            "removeStyle(n, 'display')",
            "setAttribute(n, 'hidden', true)",
            "removeAttribute(n, 'hidden')",
            "setAttribute(n, 'inert', true)",
            "removeAttribute(n, 'inert')",
            "removeAttribute(n, 'TABINDEX')",
        ] {
            engine
                .eval::<()>(&format!("__inca_native__.{call}"))
                .unwrap();
            assert!(dirty(&host).contains(&node), "{call}");
        }
        engine
            .eval::<()>("__inca_native__.setAttribute(n, 'tabIndex', 1)")
            .unwrap();
        let host = host.borrow();
        let attrs = host.tree.get(node).unwrap().attributes();
        assert_eq!(attrs.get("tabindex"), Some(&AttributeValue::Number(1.0)));
        assert!(!attrs.contains_key("tabIndex"));
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
    fn moving_the_root_raises_a_catchable_exception() {
        let (engine, host) = engine_with_bindings();

        let caught: u32 = engine
            .eval(
                r"
                const root = __inca_native__.rootNodeId();
                const n = __inca_native__.createNode('div');
                let caught = 0;
                try {
                    __inca_native__.appendChild(n, root);
                } catch (e) {
                    if (e instanceof TypeError) caught += 1;
                }
                try {
                    __inca_native__.insertBefore(n, root, null);
                } catch (e) {
                    if (e instanceof TypeError) caught += 1;
                }
                __inca_native__.destroyNode(n);
                caught;
                ",
            )
            .unwrap();

        assert_eq!(caught, 2, "the root belongs to the host, not to the app");
        let host = host.borrow();
        let root = host.tree.get(host.root).unwrap();
        assert_eq!(root.parent(), None);
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
    fn node_id_that_is_not_a_whole_u32_raises_type_error() {
        let (engine, _host) = engine_with_bindings();

        for bad in ["NaN", "1.5", "-1", "Infinity", "2 ** 32", "'1'"] {
            let script = format!(
                "(() => {{ const node = __inca_native__.createNode('div');
                 const rejects = (f) => {{
                     try {{ f(); return false; }} catch (e) {{ return e instanceof TypeError; }}
                 }};
                 return [
                     rejects(() => __inca_native__.setStyle({bad}, 'width', 1)),
                     rejects(() => __inca_native__.removeStyle({bad}, 'width')),
                     rejects(() => __inca_native__.removeAttribute({bad}, 'label')),
                     rejects(() => __inca_native__.appendChild({bad}, node)),
                     rejects(() => __inca_native__.appendChild(node, {bad})),
                     rejects(() => __inca_native__.insertBefore(node, node, {bad})),
                     rejects(() => __inca_native__.destroyNode({bad})),
                     rejects(() => __inca_native__.addEventListener({bad}, 'click', 1)),
                     rejects(() => __inca_native__.removeEventListener({bad}, 'click', 1)),
                     rejects(() => __inca_native__.focusNode({bad})),
                 ].every(Boolean); }})()"
            );
            assert!(engine.eval::<bool>(&script).unwrap(), "{bad}");
        }
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
    fn remove_attribute_unsets_the_key_and_rejects_an_unknown_node() {
        let (engine, host) = engine_with_bindings();

        let ids: Vec<u32> = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'label', 'a');
                n.setAttribute(node, 'count', 1);
                n.removeAttribute(node, 'label');
                n.removeAttribute(node, 'never_set');
                let caught = false;
                try { n.removeAttribute(999, 'label'); } catch (e) { caught = true; }
                [node, caught ? 1 : 0];
                ",
            )
            .unwrap();

        let [node, caught] = ids[..] else {
            panic!("expected the node id and the caught flag");
        };
        assert_eq!(caught, 1);
        let host = host.borrow();
        let attrs = host.tree.get(node).unwrap().attributes();
        assert!(attrs.get("label").is_none());
        assert!(attrs.get("count").is_some());
    }

    #[test]
    fn remove_style_unsets_the_key_and_rejects_an_unknown_node() {
        let (engine, host) = engine_with_bindings();

        let ids: Vec<u32> = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setStyle(node, 'gap', 8);
                n.setStyle(node, 'width', 1);
                n.removeStyle(node, 'gap');
                let caught = false;
                try { n.removeStyle(999, 'gap'); } catch (e) { caught = true; }
                [node, caught ? 1 : 0];
                ",
            )
            .unwrap();

        let [node, caught] = ids[..] else {
            panic!("expected the node id and the caught flag");
        };
        assert_eq!(caught, 1);
        let host = host.borrow();
        let styles = host.tree.get(node).unwrap().style_props();
        assert!(styles.get("gap").is_none());
        assert!(styles.get("width").is_some());
    }

    /// Evaluates `script` against bindings whose style warnings are
    /// collected, and returns them with the host.
    fn warnings_from(script: &str) -> (Rc<RefCell<Vec<String>>>, Rc<RefCell<Host>>) {
        let (engine, host) = engine_with_bindings();
        let warnings = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&warnings);
        host.borrow_mut().warn = Rc::new(move |line| sink.borrow_mut().push(line.to_owned()));
        engine.eval::<()>(script).unwrap();
        (warnings, host)
    }

    #[test]
    fn set_style_warns_once_per_unknown_key() {
        let (warnings, _) = warnings_from(
            "const n = __inca_native__.createNode('div'); \
             __inca_native__.setStyle(n, 'flexDirections', 'column');",
        );

        assert_eq!(
            *warnings.borrow(),
            ["node 1 (div): ignoring unknown style key `flexDirections`"]
        );
    }

    #[test]
    fn set_style_warns_for_each_set_of_a_bad_value() {
        let (warnings, _) = warnings_from(
            "const n = __inca_native__.createNode('div'); \
             __inca_native__.setStyle(n, 'background', -1); \
             __inca_native__.setStyle(n, 'background', -1); \
             __inca_native__.setStyle(n, 'background', 0xff0000);",
        );

        assert_eq!(
            *warnings.borrow(),
            ["node 1 (div): ignoring invalid color for style key `background`"; 2]
        );
    }

    #[test]
    fn a_failed_set_style_emits_no_warning() {
        let (warnings, _) = warnings_from(
            "try { __inca_native__.setStyle(999, 'bogus', 1); } catch (e) {} \
             const n = __inca_native__.createNode('div'); \
             try { __inca_native__.setStyle(n, 'bogus', { nested: true }); } catch (e) {}",
        );

        assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
    }

    #[test]
    fn set_style_names_the_tag_of_the_node() {
        let (warnings, _) = warnings_from(
            "const n = __inca_native__.createNode('text'); \
             __inca_native__.setStyle(n, 'bogus', 1);",
        );

        assert_eq!(
            *warnings.borrow(),
            ["node 1 (text): ignoring unknown style key `bogus`"]
        );
    }

    #[test]
    fn set_style_is_quiet_for_known_keys_and_box_keys() {
        let (warnings, _) = warnings_from(
            "const n = __inca_native__.createNode('div'); \
             for (const k of ['display', 'padding', 'padding_x', 'margin_left', 'gap']) \
               __inca_native__.setStyle(n, k, 'flex'); \
             __inca_native__.setStyle(n, 'text_color', '#fff');",
        );

        assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
    }

    #[test]
    fn a_stored_bad_style_does_not_warn_again_when_the_tree_is_read() {
        let (warnings, host) = warnings_from(
            "const n = __inca_native__.createNode('div'); \
             __inca_native__.setStyle(n, 'bogus', 1);",
        );
        assert_eq!(warnings.borrow().len(), 1);

        let host = host.borrow();
        for _ in 0..3 {
            inca_gpui::build_spec(&host.tree, 1).unwrap();
        }
        assert_eq!(warnings.borrow().len(), 1);
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
