// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Bounds, ElementId, Pixels, Point, ScrollHandle, Window, div, point, px,
};

use super::input::InputElement;
use super::pointer::{
    mark_mouse_target, pointer_tracker, root_keys, wire_focus, wire_hover, wire_modifier_keys,
};
use super::scroll::{scroll_anchor, snapped_extra, wire_scroll};
use super::spec::{DisplaySpec, ElementSpec, ElementTag, build_spec};
use super::style::{apply_style, clip_scroll_axes};
use crate::event_sink::{EventPayload, EventSink, noop_sink_methods};
use crate::tree::{NodeId, VirtualTree};

/// The selector `build_element` gives the container with this id. A test
/// passes it to `gpui`'s `debug_bounds` to read that container's bounds.
#[must_use]
pub fn debug_selector(id: NodeId) -> String {
    format!("node-{id}")
}

/// Recursively converts an [`ElementSpec`] into a real `gpui` [`AnyElement`].
///
/// With `track`, every non-inert container reports itself as the mouse target,
/// which gives it a hitbox, and reports its hover state through `on_hover`,
/// which needs a `gpui` `ElementId` (`.id()`). A scroll container gets an id too, since
/// `gpui` keeps its scroll offset in element state.
///
/// Every container carries the selector [`debug_selector`] names, a no-op
/// outside test builds, so a test can look its computed bounds up by
/// [`NodeId`].
///
/// The root container (`root`) also carries the pointer tracker and the key
/// listeners when there is a dispatcher.
fn build_element_inner<E: EventSink + Clone + 'static>(
    spec: &ElementSpec,
    dispatch: Option<&E>,
    root: bool,
    track: bool,
) -> AnyElement {
    match &spec.tag {
        ElementTag::Text(content) => content.clone().into_any_element(),
        ElementTag::Container | ElementTag::Input { .. } => {
            let id = spec.id;
            let handle = dispatch
                .filter(|_| spec.style.is_scroll_container())
                .and_then(|d| d.scroll_handle(id));
            let element = div().debug_selector(move || debug_selector(id));
            // Before `.id()`: only `Div` has the listener.
            let element = element.when_some(dispatch, |el, dispatch| {
                record_children(el, spec, handle.clone(), dispatch.clone())
            });

            let element = element.when_some(dispatch.filter(|_| root), |el, dispatch| {
                wire_modifier_keys(root_keys(el, dispatch.clone()), dispatch)
            });

            let hover = track && !spec.inert;
            if spec.style.is_scroll_container() || (dispatch.is_some() && hover) {
                let element = element.id(ElementId::Integer(u64::from(id)));
                let element = wire_focus(element, dispatch, id);
                let element = wire_scroll(element, handle.clone(), spec, root, dispatch);
                let element = wire_hover(element, dispatch.filter(|_| hover), id);
                let element =
                    element.when(hover, |el| mark_mouse_target(el, id, dispatch.cloned()));
                finish_container(element, spec, dispatch, root, track, handle.is_some())
            } else {
                finish_container(
                    wire_focus(element, dispatch, id),
                    spec,
                    dispatch,
                    root,
                    track,
                    false,
                )
            }
        }
    }
}

/// Applies `spec`'s style and children to a container, whichever kind of
/// element it turned out to be. The root container (`root`) also carries the
/// pointer tracker, as its first child, when there is a dispatcher. A scroll
/// container carries its scroll anchor after the tracker and before the
/// children.
fn finish_container<E>(
    mut element: E,
    spec: &ElementSpec,
    dispatch: Option<&(impl EventSink + Clone + 'static)>,
    root: bool,
    track: bool,
    scrolls: bool,
) -> AnyElement
where
    E: Styled + ParentElement + IntoElement + 'static,
{
    apply_style(element.style(), &spec.style);
    // The pointer tracker and the root's listener apply the wheel step of a tracked container.
    if scrolls {
        clip_scroll_axes(element.style());
    }
    if let Some(dispatch) = dispatch.filter(|_| root) {
        element = element.child(pointer_tracker(spec.id, dispatch.clone()));
    }
    if spec.style.is_scroll_container() {
        element = element.child(scroll_anchor(&spec.style));
    }
    if let ElementTag::Input {
        placeholder,
        editable,
    } = &spec.tag
    {
        element = element.child(InputElement::new(
            spec.id,
            placeholder.clone().into(),
            *editable,
            dispatch.cloned(),
        ));
    }
    for child in rendered_children(spec) {
        element = element.child(build_element_inner(child, dispatch, false, track));
    }
    element.into_any_element()
}

/// The children of `spec` that become `gpui` elements, in order. An empty
/// text child renders nothing.
fn rendered_children(spec: &ElementSpec) -> impl Iterator<Item = &ElementSpec> {
    spec.children
        .iter()
        .filter(|child| child.tag != ElementTag::Text(String::new()))
}

/// The laid-out box of a node, as the last drawn frame placed it.
///
/// `bounds` is in window coordinates, after the scrolling of every ancestor.
/// A node inside a scrolled container is therefore reported where it is
/// drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeLayout {
    /// The border box.
    pub bounds: Bounds<Pixels>,
    /// The border width, the same on all four sides. 0 when the node has no
    /// border and for text.
    pub border: Pixels,
    /// The scroll offset (positive) the parent applied to `bounds`. Adding it
    /// back gives the box as laid out before scrolling.
    pub scroll: Point<Pixels>,
}

/// Reports the box of each rendered child of `spec` to `sink` once `element`
/// has laid its children out. A child with `display: none` has no box and
/// reports nothing.
fn record_children<E>(
    element: gpui::Div,
    spec: &ElementSpec,
    handle: Option<ScrollHandle>,
    sink: E,
) -> gpui::Div
where
    E: EventSink + 'static,
{
    // The border of each rendered child; `None` for `display: none`.
    #[allow(clippy::cast_possible_truncation)]
    let children: Vec<(NodeId, Option<Pixels>)> = rendered_children(spec)
        .map(|child| {
            let shown = child.style.display != Some(DisplaySpec::None);
            let border = px(child.style.border_width.unwrap_or(0.0) as f32);
            (child.id, shown.then_some(border))
        })
        .collect();
    element.when(!children.is_empty(), |el| {
        el.on_children_prepainted(move |listed, window, _| {
            // `gpui` hands a scroll-tracked container's bounds to its handle
            // and passes the listener none.
            let raw: Vec<Bounds<Pixels>> = match &handle {
                Some(handle) => (0..).map_while(|ix| handle.bounds_for_item(ix)).collect(),
                None => listed,
            };
            let offset = handle
                .as_ref()
                .map(ScrollHandle::offset)
                .unwrap_or_default();
            // `gpui` snaps the whole accumulated offset, and the raw bounds
            // already hold the snapped offset of the ancestors.
            let outer = window.element_offset();
            let offset = snapped_extra(window, outer + offset, outer);
            // The tracker and the scroll anchor come before the children.
            let first = raw.len().saturating_sub(children.len());
            for (&(id, border), bounds) in children.iter().zip(&raw[first..]) {
                if let Some(border) = border {
                    let bounds = Bounds::new(bounds.origin + offset, bounds.size);
                    let scroll = point(-offset.x, -offset.y);
                    sink.record_layout(
                        id,
                        NodeLayout {
                            bounds,
                            border,
                            scroll,
                        },
                    );
                }
            }
        })
    })
}

/// Type-only stand-in for [`build_element_inner`]'s `dispatch` parameter
/// when there's no dispatcher at all. [`build_element`] always passes
/// `None`, so no method here is reachable, but a concrete type is still needed
/// to name in that `None`.
#[derive(Clone)]
struct NoSink;

impl EventSink for NoSink {
    fn tracks_pointer(&self) -> bool {
        false
    }

    fn fire(
        &self,
        _start: NodeId,
        _event: &str,
        _payload: &EventPayload,
        _window: &mut Window,
        _cx: &mut App,
    ) -> bool {
        false
    }

    fn focus_handle(&self, _node_id: NodeId) -> Option<gpui::FocusHandle> {
        None
    }

    fn scroll_handle(&self, _node_id: NodeId) -> Option<ScrollHandle> {
        None
    }

    fn wheel_axes(&self, _node_id: NodeId) -> Option<Point<bool>> {
        None
    }

    noop_sink_methods!(
        clear_layout,
        record_layout,
        pointer_moved,
        pointer_left,
        modifiers_changed,
        hover_changed,
        pointer_over,
        pointer_input,
        pointer_released,
        key_input,
        input,
        record_input,
        input_request,
    );
}

/// Recursively converts an [`ElementSpec`] into a real `gpui` [`AnyElement`],
/// with no event wiring. See [`build_element_with_events`] for a version
/// whose containers dispatch input events into JS.
#[must_use]
pub fn build_element(spec: &ElementSpec) -> AnyElement {
    build_element_inner::<NoSink>(spec, None, true, false)
}

/// Like [`build_element`], but input on the containers dispatches into JS
/// via `dispatch` (any type implementing this crate's [`EventSink`] trait).
#[must_use]
pub fn build_element_with_events<E: EventSink + Clone + 'static>(
    spec: &ElementSpec,
    dispatch: &E,
) -> AnyElement {
    build_element_inner(spec, Some(dispatch), true, dispatch.tracks_pointer())
}

/// Composes [`build_spec`] and [`build_element`]: builds `root` and its
/// whole subtree from `tree` into a real `gpui` element. `None` if `root`
/// doesn't resolve.
#[must_use]
pub fn render_tree(tree: &VirtualTree, root: NodeId) -> Option<AnyElement> {
    build_spec(tree, root).map(|spec| build_element(&spec))
}

/// Like [`render_tree`], but input on the containers dispatches into JS via
/// `dispatch` (see [`build_element_with_events`]).
#[must_use]
pub fn render_tree_with_events<E: EventSink + Clone + 'static>(
    tree: &VirtualTree,
    root: NodeId,
    dispatch: &E,
) -> Option<AnyElement> {
    dispatch.clear_layout();
    build_spec(tree, root).map(|spec| build_element_with_events(&spec, dispatch))
}
