// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::prelude::*;
use gpui::{AnyElement, App, Bounds, ElementId, Pixels, Point, ScrollHandle, Window, div};

use super::children::{record_children, rendered_children};
use super::input::InputElement;
use super::pointer::{
    mark_mouse_target, pointer_tracker, root_keys, wire_focus, wire_hover, wire_modifier_keys,
};
use super::scroll::{scroll_anchor, wire_scroll};
use super::spec::{ElementSpec, ElementTag, build_spec};
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::AttributeValue;
    use crate::tests::{num as n, text as s, *};
    use gpui::{
        Context, Pixels, Point, Render, ScrollDelta, ScrollWheelEvent, TestAppContext,
        VisualTestContext, point, px, size,
    };
    use inca_test_macros::gpui_cases;

    fn row() -> [(&'static str, AttributeValue); 4] {
        [
            ("display", s("flex")),
            ("flex_direction", s("row")),
            ("width", n(200.)),
            ("height", n(50.)),
        ]
    }

    struct TreeView(VirtualTree);

    impl Render for TreeView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            render_tree(&self.0, 0).unwrap()
        }
    }

    /// How far a wheel `delta` over a 100x100 container with `props`
    /// moves its 300x300 child, without a dispatcher.
    fn content_shift(
        cx: &mut TestAppContext,
        props: &[(&str, &str)],
        delta: Point<Pixels>,
    ) -> Point<Pixels> {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.set_style(root, "width", 100.0).unwrap();
        tree.set_style(root, "height", 100.0).unwrap();
        for (key, value) in props {
            tree.set_style(root, *key, *value).unwrap();
        }
        tree.set_style(child, "width", 300.0).unwrap();
        tree.set_style(child, "height", 300.0).unwrap();
        tree.append_child(root, child).unwrap();

        let window = cx.add_window(|_, _| TreeView(tree));
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(delta),
            ..Default::default()
        });
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        let container = cx.debug_bounds("node-0").unwrap();
        let content = cx.debug_bounds("node-1").unwrap();
        content.origin - container.origin
    }

    /// Draws an outer and an inner scroll container at `scale`, scrolled
    /// by `outer` and `inner`, and returns the leaf's recorded y.
    fn nested_leaf_y(cx: &mut TestAppContext, scale: f32, outer: f32, inner: f32) -> Pixels {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 100., 100.);
        tree.set_style(root, "overflow_y", "scroll").unwrap();
        let mid = boxed(&mut tree, Some(root), 100., 300.);
        tree.set_style(mid, "overflow_y", "scroll").unwrap();
        let leaf = boxed(&mut tree, Some(mid), 100., 600.);
        let (probe, window) = mounted(cx, tree);
        cx.simulate_window_scale_factor_change(window, scale);
        frame(cx, window);
        probe.handles.borrow()[&root].set_offset(point(px(0.), px(outer)));
        probe.handles.borrow()[&mid].set_offset(point(px(0.), px(inner)));
        frame(cx, window);
        probe.layouts.borrow()[&leaf].bounds.origin.y
    }

    #[gpui::test]
    fn container_with_fixed_size_lays_out_at_that_size(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        tree.set_style(root, "width", 120.0).unwrap();
        tree.set_style(root, "height", 80.0).unwrap();

        let cx = cx.add_empty_window();
        cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
            render_tree(&tree, root).unwrap()
        });

        let bounds = cx
            .debug_bounds("node-0")
            .expect("root container should be tagged with a debug selector");
        assert_eq!(bounds.size.width, px(120.0));
        assert_eq!(bounds.size.height, px(80.0));
    }

    #[gpui::test]
    fn text_leaf_renders_without_panicking(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("#text").unwrap();
        tree.set_attribute(root, "value", "hello").unwrap();

        let cx = cx.add_empty_window();
        cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
            render_tree(&tree, root).unwrap()
        });
    }

    #[gpui::test]
    fn padding_insets_the_child(cx: &mut TestAppContext) {
        let bounds = layout(
            cx,
            &[("width", n(200.)), ("height", n(100.)), ("padding", n(10.))],
            &[&[("width", n(50.)), ("height", n(30.))]],
        );
        assert_eq!(bounds[1].origin - bounds[0].origin, point(px(10.), px(10.)));
        assert_eq!(bounds[0].size, size(px(200.), px(100.)));
    }

    #[gpui::test]
    fn padding_grows_a_container_sized_by_its_content(cx: &mut TestAppContext) {
        let bounds = layout(
            cx,
            &[("padding_top", n(4.)), ("padding_bottom", n(6.))],
            &[&[("height", n(30.))]],
        );
        assert_eq!(bounds[0].size.height, px(40.));
    }

    #[gpui::test]
    fn a_negative_margin_pulls_the_child_out(cx: &mut TestAppContext) {
        let bounds = layout(
            cx,
            &[("width", n(200.)), ("height", n(100.))],
            &[&[
                ("width", n(50.)),
                ("height", n(30.)),
                ("margin_left", n(-5.)),
            ]],
        );
        assert_eq!(bounds[1].origin.x - bounds[0].origin.x, px(-5.));
    }

    #[gpui::test]
    fn margin_offsets_the_child(cx: &mut TestAppContext) {
        let bounds = layout(
            cx,
            &[("width", n(200.)), ("height", n(100.))],
            &[&[
                ("width", n(50.)),
                ("height", n(30.)),
                ("margin", n(2.)),
                ("margin_left", n(7.)),
                ("margin_top", n(3.)),
            ]],
        );
        assert_eq!(bounds[1].origin - bounds[0].origin, point(px(7.), px(3.)));
    }

    #[gpui::test]
    fn an_auto_margin_takes_the_free_space(cx: &mut TestAppContext) {
        let row = [
            ("display", s("flex")),
            ("flex_direction", s("row")),
            ("width", n(200.)),
            ("height", n(100.)),
        ];
        let pushed = layout(
            cx,
            &row,
            &[&[
                ("width", n(50.)),
                ("height", n(30.)),
                ("margin_left", s("auto")),
            ]],
        );
        assert_eq!(pushed[1].origin.x - pushed[0].origin.x, px(150.));
        let centered = layout(
            cx,
            &row,
            &[&[
                ("width", n(50.)),
                ("height", n(30.)),
                ("margin_x", s("auto")),
            ]],
        );
        assert_eq!(centered[1].origin.x - centered[0].origin.x, px(75.));
    }

    #[gpui::test]
    fn min_width_widens_and_min_height_heightens(cx: &mut TestAppContext) {
        let bounds = layout(
            cx,
            &[
                ("width", n(50.)),
                ("height", n(20.)),
                ("min_width", n(80.)),
                ("min_height", n(35.)),
            ],
            &[],
        );
        assert_eq!(bounds[0].size, size(px(80.), px(35.)));
    }

    #[gpui::test]
    fn max_width_narrows_and_max_height_shortens(cx: &mut TestAppContext) {
        let bounds = layout(
            cx,
            &[
                ("width", n(200.)),
                ("height", n(100.)),
                ("max_width", n(120.)),
                ("max_height", n(60.)),
            ],
            &[],
        );
        assert_eq!(bounds[0].size, size(px(120.), px(60.)));
    }

    #[gpui::test]
    fn flex_grow_fills_the_remaining_space(cx: &mut TestAppContext) {
        let bounds = layout(
            cx,
            &row(),
            &[
                &[("width", n(50.)), ("flex_grow", n(1.))],
                &[("width", n(50.))],
            ],
        );
        assert_eq!(bounds[1].size.width, px(150.));
        assert_eq!(bounds[2].size.width, px(50.));
        assert_eq!(bounds[2].origin.x - bounds[0].origin.x, px(150.));
    }

    #[gpui::test]
    fn an_x_only_container_scrolls_along_x_without_a_dispatcher(cx: &mut TestAppContext) {
        let shift = content_shift(
            cx,
            &[("overflow_x", "scroll"), ("overflow_y", "hidden")],
            point(px(-50.), px(0.)),
        );
        assert_eq!(shift, point(px(-50.), px(0.)));
    }

    #[gpui::test]
    fn a_scroll_container_scrolls_without_a_dispatcher(cx: &mut TestAppContext) {
        let shift = content_shift(cx, &[("overflow_y", "scroll")], point(px(0.), px(-50.)));
        assert_eq!(shift, point(px(0.), px(-50.)));
    }

    #[gpui::test]
    fn the_parents_scroll_offset_moves_the_children(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 100., 100.);
        tree.set_style(root, "overflow_y", "scroll").unwrap();
        let first = boxed(&mut tree, Some(root), 100., 300.);
        let second = boxed(&mut tree, Some(root), 100., 20.);
        let (probe, window) = mounted(cx, tree);
        frame(cx, window);
        assert_eq!(
            probe.layouts.borrow()[&first].bounds,
            rect(0., 0., 100., 300.)
        );
        probe.handles.borrow()[&root].set_offset(point(px(0.), px(-50.)));
        frame(cx, window);
        let layouts = probe.layouts.borrow();
        assert_eq!(layouts[&first].bounds, rect(0., -50., 100., 300.));
        assert_eq!(layouts[&second].bounds, rect(0., 250., 100., 20.));
    }

    #[gpui::test]
    fn a_hidden_container_ignores_the_wheel(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 100., 100.);
        tree.set_style(root, "overflow_x", "hidden").unwrap();
        tree.set_style(root, "overflow_y", "hidden").unwrap();
        let _child = boxed(&mut tree, Some(root), 100., 300.);
        let (probe, window) = mounted(cx, tree);
        frame(cx, window);
        let mut vcx = VisualTestContext::from_window(window, cx);
        vcx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(10.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(-50.))),
            ..Default::default()
        });
        vcx.update(|window, cx| window.draw(cx).clear(cx));
        let handle = probe.handles.borrow()[&root].clone();
        assert_eq!(handle.offset(), point(px(0.), px(0.)));
    }

    #[gpui::test]
    fn a_hidden_container_moves_its_content_by_a_script_offset(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 100., 100.);
        tree.set_style(root, "overflow_x", "hidden").unwrap();
        tree.set_style(root, "overflow_y", "hidden").unwrap();
        let child = boxed(&mut tree, Some(root), 100., 300.);
        let (probe, window) = mounted(cx, tree);
        frame(cx, window);
        let mut vcx = VisualTestContext::from_window(window, cx);
        let handle = probe.handles.borrow()[&root].clone();
        handle.set_offset(point(px(0.), px(-50.)));
        vcx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(handle.max_offset().y, px(200.));
        assert_eq!(
            probe.layouts.borrow()[&child].bounds,
            rect(0., -50., 100., 300.)
        );
    }

    #[gpui::test]
    fn a_bordered_scroll_range_ends_at_the_padding(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 400., 400.);
        let scroller = boxed(&mut tree, Some(root), 120., 100.);
        tree.set_style(scroller, "padding", 10.0).unwrap();
        tree.set_style(scroller, "border_width", 3.0).unwrap();
        tree.set_style(scroller, "overflow", "scroll").unwrap();
        for _ in 0..2 {
            let child = boxed(&mut tree, Some(scroller), 300., 120.);
            tree.set_style(child, "flex_shrink", 0.0).unwrap();
        }
        let (probe, window) = mounted(cx, tree);
        frame(cx, window);
        let handle = probe.handles.borrow()[&scroller].clone();
        assert_eq!(handle.max_offset(), point(px(206.), px(166.)));
    }

    #[gpui::test]
    fn the_scroll_range_holds_the_start_margins_of_the_first_child(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 400., 400.);
        let scroller = boxed(&mut tree, Some(root), 120., 100.);
        tree.set_style(scroller, "padding", 10.0).unwrap();
        tree.set_style(scroller, "overflow", "scroll").unwrap();
        let child = boxed(&mut tree, Some(scroller), 300., 240.);
        tree.set_style(child, "margin_top", 7.0).unwrap();
        tree.set_style(child, "margin_left", 7.0).unwrap();
        let (probe, window) = mounted(cx, tree);
        frame(cx, window);
        let handle = probe.handles.borrow()[&scroller].clone();
        assert_eq!(handle.max_offset(), point(px(207.), px(167.)));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::below_half(-50.4, -50.5)]
    #[case::quarter(-50.25, -50.)]
    #[case::three_quarters(-50.75, -50.5)]
    fn a_fractional_scroll_offset_is_snapped_to_the_device_pixel(
        cx: &mut TestAppContext,
        offset: f32,
        drawn: f32,
    ) {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 100., 100.);
        tree.set_style(root, "overflow_y", "scroll").unwrap();
        let child = boxed(&mut tree, Some(root), 100., 300.);
        let (probe, window) = mounted(cx, tree);
        // The test window has a scale factor of 2, so the grid is 0.5px.
        frame(cx, window);
        probe.handles.borrow()[&root].set_offset(point(px(0.), px(offset)));
        frame(cx, window);
        assert_eq!(
            probe.layouts.borrow()[&child].bounds,
            rect(0., drawn, 100., 300.)
        );
    }

    #[gpui::test]
    fn nested_fractional_offsets_are_snapped_as_one_sum(cx: &mut TestAppContext) {
        // Drawn at the snap of the summed offset.
        assert_eq!(nested_leaf_y(cx, 2., -10.3, -10.3), px(-20.5));
        assert_eq!(nested_leaf_y(cx, 1., -10.3, -10.3), px(-21.));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::rounds_down(-50.4, -50.)]
    #[case::rounds_up(-50.6, -51.)]
    fn a_fractional_scroll_offset_snaps_to_whole_pixels_at_scale_one(
        cx: &mut TestAppContext,
        offset: f32,
        drawn: f32,
    ) {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 100., 100.);
        tree.set_style(root, "overflow_y", "scroll").unwrap();
        let child = boxed(&mut tree, Some(root), 100., 300.);
        let (probe, window) = mounted(cx, tree);
        cx.simulate_window_scale_factor_change(window, 1.);
        frame(cx, window);
        probe.handles.borrow()[&root].set_offset(point(px(0.), px(offset)));
        frame(cx, window);
        assert_eq!(
            probe.layouts.borrow()[&child].bounds,
            rect(0., drawn, 100., 300.)
        );
    }
}
