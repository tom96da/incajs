// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds `globalThis.__inca_native__`: the small set of native functions
//! JS calls to read the root handle, mutate the retained virtual tree,
//! register input-event callbacks, dispatch events from script, and free what
//! it no longer needs.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{Bounds, ScrollHandle, px};
use rquickjs::function::Opt;
use rquickjs::{Ctx, Exception, FromJs, Function, IntoJs, Object, Result as JsResult, Value};

use inca_jsenv::EngineError;

use inca_gpui::{
    AttributeValue, NodeId, NodeLayout, TreeError, VirtualNode, VirtualTree, is_scroll_container,
    style_warning,
};

use crate::focus::FocusRegistry;
use crate::scroll_into_view::Align;

/// One registration: the callback id, the phase it listens in and whether it
/// is passive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listener {
    /// The id the JS caller stored its function under.
    pub callback: u32,
    /// Listens while the event travels down to the target.
    pub capture: bool,
    /// `preventDefault()` calls from the callback are ignored.
    pub passive: bool,
}

/// Every `(node id, event name)` a JS caller has registered through
/// `addEventListener`, mapped to the [`Listener`]s it gave us. Holds owned
/// Rust data only, so Rust code outside any JS call, such as the input-event
/// dispatcher, can read it.
///
/// Nothing expires on its own: a registration lives until
/// [`unregister`](Self::unregister) drops it, or [`release`](Self::release)
/// does when the node goes away.
#[derive(Debug, Default)]
pub struct EventListeners {
    by_node: HashMap<NodeId, HashMap<String, Vec<Listener>>>,
}

impl EventListeners {
    /// Adds `callback_id` to `(node_id, event)` as a bubble-phase, active
    /// listener. Registering an id already there leaves the registrations
    /// unchanged.
    pub fn register(&mut self, node_id: NodeId, event: impl Into<String>, callback_id: u32) {
        self.register_with(node_id, event, callback_id, false, false);
    }

    /// Adds `callback_id` to `(node_id, event)` with the given phase and
    /// passivity. Registering an id already there leaves the registrations
    /// unchanged.
    pub fn register_with(
        &mut self,
        node_id: NodeId,
        event: impl Into<String>,
        callback_id: u32,
        capture: bool,
        passive: bool,
    ) {
        let listeners = self
            .by_node
            .entry(node_id)
            .or_default()
            .entry(event.into())
            .or_default();
        if !listeners.iter().any(|l| l.callback == callback_id) {
            listeners.push(Listener {
                callback: callback_id,
                capture,
                passive,
            });
        }
    }

    /// Drops `callback_id` from `(node_id, event)`, and reports whether it
    /// was registered.
    pub fn unregister(&mut self, node_id: NodeId, event: &str, callback_id: u32) -> bool {
        let Some(events) = self.by_node.get_mut(&node_id) else {
            return false;
        };
        let Some(listeners) = events.get_mut(event) else {
            return false;
        };
        let before = listeners.len();
        listeners.retain(|l| l.callback != callback_id);
        let removed = listeners.len() != before;
        if listeners.is_empty() {
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
            .map(|events| events.into_values().flatten().map(|l| l.callback).collect())
            .unwrap_or_default()
    }

    /// Whether nothing is registered anywhere.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_node.is_empty()
    }

    /// The callback ids of `(node_id, event)` in registration order.
    #[must_use]
    pub fn callbacks_for(&self, node_id: NodeId, event: &str) -> Vec<u32> {
        self.listeners_for(node_id, event)
            .iter()
            .map(|l| l.callback)
            .collect()
    }

    /// The registrations of `(node_id, event)` in registration order.
    #[must_use]
    pub fn listeners_for(&self, node_id: NodeId, event: &str) -> &[Listener] {
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
    /// Last number handed out as an `eventId`. Each event takes the next one.
    pub last_event_id: u64,
    // Scroll state of the containers, created on first ask. Its own cell, so
    // it can be written while a shared borrow of `Host` is held.
    pub(crate) scroll_handles: RefCell<HashMap<NodeId, ScrollHandle>>,
    // The offset of each container as of the last `scroll` check.
    scroll_reported: HashMap<NodeId, gpui::Point<gpui::Pixels>>,
    // Boxes of the last drawn frame. Its own cell, as the draw runs under a shared borrow.
    pub(crate) layout: RefCell<HashMap<NodeId, NodeLayout>>,
    // Errors listeners threw, in order; the dispatcher reports them once the engine is idle.
    pub(crate) failures: Vec<EngineError>,
    // The nodes whose `click()` is running.
    pub(crate) clicking: HashSet<NodeId>,
    // `scrollIntoViewNode` calls whose node had no box yet: awaiting the next
    // `reveal_focus`, and awaiting the frame after it.
    scroll_fresh: Vec<(NodeId, Align, Align)>,
    scroll_retry: Vec<(NodeId, Align, Align)>,
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
            last_event_id: 0,
            scroll_handles: RefCell::new(HashMap::new()),
            scroll_reported: HashMap::new(),
            layout: RefCell::new(HashMap::new()),
            failures: Vec::new(),
            clicking: HashSet::new(),
            scroll_fresh: Vec::new(),
            scroll_retry: Vec::new(),
        }
    }
}

impl Host {
    // Containers whose clamped offset moved since the last call and whose
    // `scroll` a listener reaches: one on the container, or a capture one on
    // an ancestor. Every container's offset is recorded, so a listener added
    // later sees only later movement.
    pub(crate) fn scrolled_nodes(&mut self) -> Vec<NodeId> {
        let mut moved = Vec::new();
        for (&id, handle) in self.scroll_handles.get_mut().iter() {
            let (offset, max) = (handle.offset(), handle.max_offset());
            let now = gpui::point(
                offset.x.clamp(-max.x, px(0.)),
                offset.y.clamp(-max.y, px(0.)),
            );
            let before = self.scroll_reported.insert(id, now).unwrap_or_default();
            if before != now {
                moved.push(id);
            }
        }
        let flags = crate::dispatch::spec_of("scroll").flags;
        let host: &Host = self;
        moved.retain(|&id| {
            let path = crate::event::path_from(host, id);
            crate::event::listened(host, "scroll", flags, &path, &[])
        });
        moved.sort_unstable();
        moved
    }

    /// Scrolls the scroll containers above the node whose focus request asked
    /// for it, using the boxes of the last drawn frame. A node whose box the
    /// frame lacks gets one more frame to receive it. It also runs the
    /// `scrollIntoViewNode` calls that waited for a box, with the same one-frame
    /// retry. Call it once per frame after `FocusRegistry::apply_pending`.
    pub fn reveal_focus(&mut self, window: &mut gpui::Window) {
        let (retries, fresh) = self.focus.take_reveals();
        for node in retries {
            crate::scroll_into_view::reveal(self, node, Align::Reveal, Align::CenterOutside);
        }
        let mut wait = false;
        for node in fresh {
            if !crate::scroll_into_view::reveal(self, node, Align::Reveal, Align::CenterOutside) {
                self.focus.retry_reveal(node);
                wait = true;
            }
        }
        wait |= self.reveal_scrolls();
        if wait {
            window.request_animation_frame();
        }
    }

    // Runs the queued `scrollIntoViewNode` calls. A call whose node still has
    // no box waits one more frame. Returns whether one waits.
    fn reveal_scrolls(&mut self) -> bool {
        for (node, block, inline) in std::mem::take(&mut self.scroll_retry) {
            crate::scroll_into_view::reveal(self, node, block, inline);
        }
        for (node, block, inline) in std::mem::take(&mut self.scroll_fresh) {
            if !crate::scroll_into_view::reveal(self, node, block, inline) {
                self.scroll_retry.push((node, block, inline));
            }
        }
        !self.scroll_retry.is_empty()
    }

    // Returns a clone of the node's handle, creating it on first ask.
    pub(crate) fn scroll_handle(&self, node_id: NodeId) -> ScrollHandle {
        self.scroll_handles
            .borrow_mut()
            .entry(node_id)
            .or_default()
            .clone()
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

// A missing key converts to `null`.
fn attribute_value_to_js<'js>(
    ctx: &Ctx<'js>,
    value: Option<&AttributeValue>,
) -> JsResult<Value<'js>> {
    match value {
        None => Ok(Value::new_null(ctx.clone())),
        Some(AttributeValue::String(s)) => s.as_str().into_js(ctx),
        Some(AttributeValue::Number(n)) => n.into_js(ctx),
        Some(AttributeValue::Bool(b)) => b.into_js(ctx),
    }
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
            "getAttribute",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      JsNodeId(node_id): JsNodeId,
                      key: String|
                      -> JsResult<Value<'js>> {
                    let key = canonical_attribute_key(key);
                    let host = host.borrow();
                    let node = host
                        .tree
                        .get(node_id)
                        .ok_or_else(|| throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)))?;
                    attribute_value_to_js(&ctx, node.attributes().get(&key))
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
                    for id in &freed {
                        host.scroll_handles.get_mut().remove(id);
                        host.scroll_reported.remove(id);
                    }
                    Ok(freed
                        .into_iter()
                        .flat_map(|id| {
                            host.focus.forget(id);
                            host.scroll_fresh.retain(|&(node, ..)| node != id);
                            host.scroll_retry.retain(|&(node, ..)| node != id);
                            host.listeners.release(id)
                        })
                        .collect())
                },
            )?,
        )?;
    }

    Ok(())
}

/// Registering and dropping input-event callbacks, and dispatching events
/// from script.
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
                      callback_id: u32,
                      capture: Opt<bool>,
                      passive: Opt<bool>|
                      -> JsResult<()> {
                    let mut host = host.borrow_mut();
                    if host.tree.get(node_id).is_none() {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    }
                    host.listeners.register_with(
                        node_id,
                        event,
                        callback_id,
                        capture.0.unwrap_or(false),
                        passive.0.unwrap_or(false),
                    );
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

    {
        let host = Rc::clone(host);
        native.set(
            "dispatchEvent",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      JsNodeId(node_id): JsNodeId,
                      event: Value<'js>|
                      -> JsResult<bool> {
                    if host.borrow().tree.get(node_id).is_none() {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    }
                    crate::event::dispatch_event(&ctx, &host, node_id, event)
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "click",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, JsNodeId(node_id): JsNodeId| {
                    crate::event::click(&ctx, &host, node_id);
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
            Function::new(
                ctx.clone(),
                move |JsNodeId(node_id): JsNodeId, prevent_scroll: Opt<bool>| {
                    let mut host = host.borrow_mut();
                    // Focusing a node that is gone is a no-op.
                    if host.tree.get(node_id).is_some() {
                        host.focus
                            .request_focus(node_id, prevent_scroll.0 != Some(true));
                    }
                },
            )?,
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

    {
        let host = Rc::clone(host);
        native.set(
            "focusedNode",
            Function::new(ctx.clone(), move |ctx: Ctx<'js>| -> JsResult<Value<'js>> {
                let host = host.borrow();
                match host.focus.active_node(&host.tree) {
                    Some(node_id) => node_id.into_js(&ctx),
                    None => Ok(Value::new_null(ctx.clone())),
                }
            })?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "layoutOf",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, JsNodeId(node_id): JsNodeId| -> JsResult<Value<'js>> {
                    let host = host.borrow();
                    if host.tree.get(node_id).is_none() {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    }
                    let Some(layout) = host.layout.borrow().get(&node_id).copied() else {
                        return Ok(Value::new_null(ctx.clone()));
                    };
                    let Bounds { origin, size } = layout.bounds;
                    [
                        origin.x,
                        origin.y,
                        size.width,
                        size.height,
                        layout.border,
                        layout.scroll.x,
                        layout.scroll.y,
                    ]
                    .map(|px| f64::from(f32::from(px)))
                    .into_js(&ctx)
                },
            )?,
        )?;
    }

    Ok(())
}

fn install_scroll<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    native: &Object<'js>,
) -> JsResult<()> {
    {
        let host = Rc::clone(host);
        native.set(
            "scrollOf",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, JsNodeId(node_id): JsNodeId| -> JsResult<Value<'js>> {
                    let host = host.borrow();
                    let Some(node) = host.tree.get(node_id) else {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    };
                    if !is_scroll_container(node) {
                        return Ok(Value::new_null(ctx.clone()));
                    }
                    let handle = host.scroll_handle(node_id);
                    let (offset, max) = (handle.offset(), handle.max_offset());
                    // gpui clamps on the next draw; report the same value.
                    [
                        -offset.x.clamp(-max.x, px(0.)),
                        -offset.y.clamp(-max.y, px(0.)),
                        max.x,
                        max.y,
                    ]
                    .map(|px| f64::from(f32::from(px)))
                    .into_js(&ctx)
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "scrollNode",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      JsNodeId(node_id): JsNodeId,
                      left: Option<f64>,
                      top: Option<f64>|
                      -> JsResult<()> {
                    let host = host.borrow();
                    let Some(node) = host.tree.get(node_id) else {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    };
                    if !is_scroll_container(node) {
                        return Ok(());
                    }
                    let handle = host.scroll_handle(node_id);
                    let mut offset = handle.offset();
                    // Narrowing is fine for pixel offsets; the upper clamp is gpui's.
                    #[allow(clippy::cast_possible_truncation)]
                    let negated = |v: f64| px(-(v.max(0.0) as f32));
                    if let Some(left) = left {
                        offset.x = negated(left);
                    }
                    if let Some(top) = top {
                        offset.y = negated(top);
                    }
                    let (before, max) = (handle.offset(), handle.max_offset());
                    let clamped = |v: gpui::Pixels, max: gpui::Pixels| {
                        -f32::from(v).clamp(-f32::from(max), 0.0)
                    };
                    handle.set_offset(offset);
                    // Recorded boxes follow the new offset, so a reveal in the same tick
                    // starts from where the content now is.
                    crate::scroll_into_view::shift_descendants(
                        &host,
                        node_id,
                        clamped(offset.x, max.x) - clamped(before.x, max.x),
                        clamped(offset.y, max.y) - clamped(before.y, max.y),
                    );
                    Ok(())
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "scrollIntoViewNode",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      JsNodeId(node_id): JsNodeId,
                      block: String,
                      inline: String|
                      -> JsResult<()> {
                    let align = |name: &str| match name {
                        "start" => Ok(Align::Start),
                        "center" => Ok(Align::Center),
                        "end" => Ok(Align::End),
                        "nearest" => Ok(Align::Nearest),
                        _ => Err(Exception::throw_type(
                            &ctx,
                            &format!("unknown scroll alignment: {name}"),
                        )),
                    };
                    let (block, inline) = (align(&block)?, align(&inline)?);
                    let mut host = host.borrow_mut();
                    if host.tree.get(node_id).is_none() {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    }
                    if !crate::scroll_into_view::reveal(&host, node_id, block, inline) {
                        host.scroll_fresh.push((node_id, block, inline));
                    }
                    Ok(())
                },
            )?,
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
    install_scroll(ctx, host, &native)?;
    crate::event::install(ctx)?;
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
    fn a_registration_keeps_its_phase_and_passivity_and_the_first_one_wins() {
        let mut listeners = EventListeners::default();
        listeners.register_with(1, "wheel", 7, true, true);
        listeners.register_with(1, "wheel", 7, false, false);
        listeners.register(1, "wheel", 8);

        assert_eq!(
            listeners.listeners_for(1, "wheel"),
            &[
                Listener {
                    callback: 7,
                    capture: true,
                    passive: true
                },
                Listener {
                    callback: 8,
                    capture: false,
                    passive: false
                },
            ]
        );
    }

    #[test]
    fn add_event_listener_takes_optional_capture_and_passive_flags() {
        let (engine, host) = engine_with_bindings();
        engine
            .eval::<()>(
                "const n = __inca_native__.createNode('div'); \
                 __inca_native__.addEventListener(n, 'a', 1); \
                 __inca_native__.addEventListener(n, 'a', 2, true); \
                 __inca_native__.addEventListener(n, 'a', 3, false, true); \
                 __inca_native__.addEventListener(n, 'a', 4, true, true); \
                 __inca_native__.removeEventListener(n, 'a', 4);",
            )
            .unwrap();
        let host = host.borrow();
        let flags: Vec<_> = host
            .listeners
            .listeners_for(1, "a")
            .iter()
            .map(|l| (l.callback, l.capture, l.passive))
            .collect();
        assert_eq!(
            flags,
            [(1, false, false), (2, true, false), (3, false, true)]
        );
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
    fn destroy_node_frees_the_scroll_handles_of_the_subtree_only() {
        let (engine, host) = engine_with_bindings();
        let ids: Vec<NodeId> = engine
            .eval(
                r"
                const root = __inca_native__.rootNodeId();
                const branch = __inca_native__.createNode('div');
                const leaf = __inca_native__.createNode('div');
                const kept = __inca_native__.createNode('div');
                __inca_native__.appendChild(root, branch);
                __inca_native__.appendChild(branch, leaf);
                __inca_native__.appendChild(root, kept);
                [branch, leaf, kept]
                ",
            )
            .unwrap();
        let (branch, leaf, kept) = (ids[0], ids[1], ids[2]);
        for id in [branch, leaf, kept] {
            host.borrow().scroll_handle(id);
        }

        engine
            .eval::<()>(&format!("__inca_native__.destroyNode({branch})"))
            .unwrap();

        let host = host.borrow();
        let handles = host.scroll_handles.borrow();
        assert!(!handles.contains_key(&branch));
        assert!(
            !handles.contains_key(&leaf),
            "a descendant's handle goes with its ancestor"
        );
        assert!(
            handles.contains_key(&kept),
            "a surviving node keeps its handle"
        );
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
    fn get_attribute_reads_the_canonical_key() {
        let (engine, _host) = engine_with_bindings();

        let same: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'tabIndex', 2);
                n.getAttribute(node, 'tabindex') === 2 &&
                    n.getAttribute(node, 'TABINDEX') === 2;
                ",
            )
            .unwrap();

        assert!(same, "attribute names are case-insensitive for tabindex");
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
    fn get_attribute_on_an_unknown_node_raises_type_error() {
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

    fn layout(x: f32) -> NodeLayout {
        NodeLayout {
            bounds: Bounds::new(
                gpui::point(gpui::px(x), gpui::px(2.0)),
                gpui::size(gpui::px(3.0), gpui::px(4.5)),
            ),
            border: gpui::px(1.0),
            scroll: gpui::point(gpui::px(6.0), gpui::px(7.5)),
        }
    }

    #[test]
    fn layout_of_reads_the_recorded_box() {
        let (engine, host) = engine_with_bindings();
        let node: u32 = engine.eval("__inca_native__.createNode('div')").unwrap();
        host.borrow().layout.borrow_mut().insert(node, layout(1.5));

        let read: Vec<f64> = engine
            .eval(&format!("__inca_native__.layoutOf({node})"))
            .unwrap();

        assert_eq!(read, [1.5, 2.0, 3.0, 4.5, 1.0, 6.0, 7.5]);
    }

    #[test]
    fn layout_of_a_node_with_no_record_is_null() {
        let (engine, _host) = engine_with_bindings();
        let node: u32 = engine.eval("__inca_native__.createNode('div')").unwrap();

        let is_null: bool = engine
            .eval(&format!("__inca_native__.layoutOf({node}) === null"))
            .unwrap();

        assert!(is_null);
    }

    #[test]
    fn layout_of_an_unknown_node_raises_type_error() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try { __inca_native__.layoutOf(999); } catch (e) { caught = e instanceof TypeError; }
                caught;
                ",
            )
            .unwrap();

        assert!(caught);
    }

    #[test]
    fn scroll_node_writes_the_given_axes_negated_and_clamps_below_zero() {
        let (engine, host) = engine_with_bindings();
        let node: u32 = engine
            .eval("globalThis.n = __inca_native__.createNode('div')")
            .unwrap();
        engine
            .eval::<()>("__inca_native__.setStyle(n, 'overflow', 'scroll')")
            .unwrap();
        let offset = |host: &Rc<RefCell<Host>>| {
            let offset = host.borrow().scroll_handle(node).offset();
            (f32::from(offset.x), f32::from(offset.y))
        };

        engine
            .eval::<()>("__inca_native__.scrollNode(n, 5, 7)")
            .unwrap();
        assert_eq!(offset(&host), (-5., -7.));
        engine
            .eval::<()>("__inca_native__.scrollNode(n, null, 9)")
            .unwrap();
        assert_eq!(offset(&host), (-5., -9.));
        engine
            .eval::<()>("__inca_native__.scrollNode(n, 3, undefined)")
            .unwrap();
        assert_eq!(offset(&host), (-3., -9.));
        engine
            .eval::<()>("__inca_native__.scrollNode(n, -4, -1)")
            .unwrap();
        assert_eq!(offset(&host), (0., 0.));
    }

    #[test]
    fn scroll_of_and_scroll_node_raise_type_error_for_an_unknown_node() {
        let (engine, _host) = engine_with_bindings();

        let caught: Vec<bool> = engine
            .eval(
                r"
                const caught = (f) => { try { f(); } catch (e) { return e instanceof TypeError; } return false; };
                [caught(() => __inca_native__.scrollOf(999)), caught(() => __inca_native__.scrollNode(999, 1, 1))];
                ",
            )
            .unwrap();

        assert_eq!(caught, [true, true]);
    }

    #[test]
    fn a_scroll_into_view_of_a_boxless_node_retries_on_one_frame() {
        let (engine, host) = engine_with_bindings();
        engine
            .eval::<()>("globalThis.n = __inca_native__.createNode('div'); __inca_native__.scrollIntoViewNode(n, 'start', 'start')")
            .unwrap();
        let mut host = host.borrow_mut();

        assert!(host.reveal_scrolls());
        assert!(!host.reveal_scrolls());
    }

    #[test]
    fn destroying_a_node_drops_its_waiting_scroll_into_view() {
        let (engine, host) = engine_with_bindings();
        engine
            .eval::<()>(
                "const n = __inca_native__; const node = n.createNode('div'); \
                 n.appendChild(0, node); n.scrollIntoViewNode(node, 'start', 'start'); \
                 n.destroyNode(node);",
            )
            .unwrap();

        assert!(!host.borrow_mut().reveal_scrolls());
    }

    #[test]
    fn scroll_into_view_node_rejects_an_unknown_node_and_an_unknown_alignment() {
        let (engine, _host) = engine_with_bindings();

        let caught: Vec<bool> = engine
            .eval(
                r"
                const caught = (f) => { try { f(); } catch (e) { return e instanceof TypeError; } return false; };
                const n = __inca_native__;
                const node = n.createNode('div');
                [
                    caught(() => n.scrollIntoViewNode(999, 'start', 'start')),
                    caught(() => n.scrollIntoViewNode(node, 'middle', 'start')),
                    caught(() => n.scrollIntoViewNode(node, 'start', 'middle')),
                    caught(() => n.scrollIntoViewNode(node, 'start', 'nearest')),
                ];
                ",
            )
            .unwrap();

        assert_eq!(caught, [true, true, true, false]);
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
                     rejects(() => __inca_native__.getAttribute({bad}, 'label')),
                     rejects(() => __inca_native__.removeAttribute({bad}, 'label')),
                     rejects(() => __inca_native__.appendChild({bad}, node)),
                     rejects(() => __inca_native__.appendChild(node, {bad})),
                     rejects(() => __inca_native__.insertBefore(node, node, {bad})),
                     rejects(() => __inca_native__.destroyNode({bad})),
                     rejects(() => __inca_native__.addEventListener({bad}, 'click', 1)),
                     rejects(() => __inca_native__.removeEventListener({bad}, 'click', 1)),
                     rejects(() => __inca_native__.focusNode({bad})),
                     rejects(() => __inca_native__.dispatchEvent({bad}, {{}})),
                     rejects(() => __inca_native__.click({bad})),
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

    #[test]
    fn focused_node_follows_focus_node_and_blur_node() {
        let (engine, _host) = engine_with_bindings();

        let flags: Vec<bool> = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                const other = n.createNode('div');
                const plain = n.createNode('div');
                n.setAttribute(node, 'tabindex', 0);
                n.setAttribute(other, 'tabindex', 0);
                const before = n.focusedNode() === null;
                n.focusNode(plain);
                const plainSkipped = n.focusedNode() === null;
                n.focusNode(node);
                const focused = n.focusedNode() === node;
                n.blurNode(other);
                const kept = n.focusedNode() === node;
                n.blurNode(node);
                [before, plainSkipped, focused, kept, n.focusedNode() === null];
                ",
            )
            .unwrap();

        assert_eq!(flags, [true; 5]);
    }
}
