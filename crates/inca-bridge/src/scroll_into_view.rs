// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Scrolls the ancestors of a node until the node shows, from the boxes of
//! the last drawn frame.

use gpui::{Bounds, Pixels, point, px};
use inca_gpui::{NodeId, is_scroll_container};

use crate::bindings::Host;

/// A box in window coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Where the node's edge or centre lands in the visible box, per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// The start edges meet.
    Start,
    /// The centres meet.
    Center,
    /// The end edges meet.
    End,
    /// A node that shows stays put. Any other node moves the least that
    /// brings its near edge in. A node longer than the box aligns its start,
    /// unless it already covers the whole box.
    Nearest,
    /// A node that shows stays put. A node that lies outside the box
    /// centres. A node that shows partly follows `Nearest`.
    Reveal,
    /// A node that shows fully or partly stays put. A node that lies outside
    /// the box centres.
    CenterOutside,
}

fn axis_delta(node_start: f32, node_len: f32, view_start: f32, view_len: f32, align: Align) -> f32 {
    let (node_end, view_end) = (node_start + node_len, view_start + view_len);
    let center = (node_start + node_len / 2.0) - (view_start + view_len / 2.0);
    let nearest = || {
        if node_start < view_start && node_end > view_end {
            0.0
        } else if node_len > view_len || node_start < view_start {
            node_start - view_start
        } else if node_end > view_end {
            node_end - view_end
        } else {
            0.0
        }
    };
    match align {
        Align::Start => node_start - view_start,
        Align::End => node_end - view_end,
        Align::Center => center,
        Align::Nearest => nearest(),
        Align::Reveal | Align::CenterOutside => {
            if node_start >= view_start && node_end <= view_end {
                0.0
            } else if node_end <= view_start || node_start >= view_end {
                center
            } else if align == Align::Reveal {
                nearest()
            } else {
                0.0
            }
        }
    }
}

/// The scroll change `(dx, dy)` that moves `node` to `inline` (horizontal)
/// and `block` (vertical) alignment in `view`. A positive value scrolls
/// towards the end of the axis.
#[must_use]
pub fn scroll_delta(node: Rect, view: Rect, block: Align, inline: Align) -> (f32, f32) {
    (
        axis_delta(node.x, node.width, view.x, view.width, inline),
        axis_delta(node.y, node.height, view.y, view.height, block),
    )
}

/// Scrolls each container of `chain`, innermost first, to show `node`.
/// `chain` pairs a container with its visible box. `scroll` receives the
/// container and the wanted change, and returns the change it applied. The
/// node moves by the applied change before the next container is handled.
pub fn scroll_chain<I: Copy>(
    mut node: Rect,
    chain: &[(I, Rect)],
    block: Align,
    inline: Align,
    mut scroll: impl FnMut(I, f32, f32) -> (f32, f32),
) {
    for &(container, view) in chain {
        let (dx, dy) = scroll_delta(node, view, block, inline);
        let (applied_x, applied_y) = scroll(container, dx, dy);
        node.x -= applied_x;
        node.y -= applied_y;
    }
}

fn rect_of(bounds: Bounds<Pixels>, inset: Pixels) -> Rect {
    Rect {
        x: f32::from(bounds.origin.x + inset),
        y: f32::from(bounds.origin.y + inset),
        width: f32::from(bounds.size.width - inset * 2.0).max(0.0),
        height: f32::from(bounds.size.height - inset * 2.0).max(0.0),
    }
}

/// Scrolls every scroll container above `node` so the node shows. Returns
/// false when the last frame recorded no box for the node.
pub(crate) fn reveal(host: &Host, node: NodeId, block: Align, inline: Align) -> bool {
    let (own, chain) = {
        let layout = host.layout.borrow();
        let Some(own) = layout.get(&node) else {
            return false;
        };
        let mut chain = Vec::new();
        let mut cursor = host.tree.get(node).and_then(inca_gpui::VirtualNode::parent);
        while let Some((id, ancestor)) = cursor.and_then(|id| host.tree.get(id).map(|n| (id, n))) {
            if is_scroll_container(ancestor)
                && let Some(l) = layout.get(&id)
            {
                chain.push((id, rect_of(l.bounds, l.border)));
            }
            cursor = ancestor.parent();
        }
        (rect_of(own.bounds, px(0.0)), chain)
    };
    scroll_chain(own, &chain, block, inline, |id, dx, dy| {
        let handle = host.scroll_handle(id);
        let (offset, max) = (handle.offset(), handle.max_offset());
        let step = |wanted: f32, offset: Pixels, max: Pixels| {
            let from = -f32::from(offset).clamp(-f32::from(max), 0.0);
            let to = (from + wanted).clamp(0.0, f32::from(max));
            (to, to - from)
        };
        let (mut x, mut y) = (offset.x, offset.y);
        let (mut applied_x, mut applied_y) = (0.0, 0.0);
        if dx != 0.0 {
            let (to, applied) = step(dx, offset.x, max.x);
            (x, applied_x) = (px(-to), applied);
        }
        if dy != 0.0 {
            let (to, applied) = step(dy, offset.y, max.y);
            (y, applied_y) = (px(-to), applied);
        }
        handle.set_offset(point(x, y));
        shift_descendants(host, id, applied_x, applied_y);
        (applied_x, applied_y)
    });
    true
}

// Moves the recorded boxes below `container` by the scroll just applied, so a
// second reveal before the next draw starts from where the content now is. The
// direct children also take it into their recorded scroll offset.
fn shift_descendants(host: &Host, container: NodeId, dx: f32, dy: f32) {
    let mut layout = host.layout.borrow_mut();
    let mut stack = vec![container];
    while let Some(id) = stack.pop() {
        let Some(node) = host.tree.get(id) else {
            continue;
        };
        stack.extend(node.children());
        if id != container
            && let Some(l) = layout.get_mut(&id)
        {
            l.bounds.origin.x -= px(dx);
            l.bounds.origin.y -= px(dy);
            if host.tree.get(id).and_then(inca_gpui::VirtualNode::parent) == Some(container) {
                l.scroll.x += px(dx);
                l.scroll.y += px(dy);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unwrap_used)]
mod tests {
    use super::*;

    const fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    // The box spans 100..200 on both axes.
    const VIEW: Rect = rect(100.0, 100.0, 100.0, 100.0);

    fn dy(top: f32, height: f32, block: Align) -> f32 {
        scroll_delta(rect(100.0, top, 10.0, height), VIEW, block, Align::Nearest).1
    }

    #[test]
    fn start_end_and_center_align_the_matching_edges() {
        assert_eq!(dy(130.0, 20.0, Align::Start), 30.0);
        assert_eq!(dy(130.0, 20.0, Align::End), -50.0);
        assert_eq!(dy(130.0, 20.0, Align::Center), -10.0);
    }

    #[test]
    fn nearest_keeps_a_node_that_shows() {
        assert_eq!(dy(130.0, 20.0, Align::Nearest), 0.0);
        assert_eq!(dy(100.0, 100.0, Align::Nearest), 0.0);
    }

    #[test]
    fn nearest_brings_the_near_edge_in_for_a_node_outside_the_box() {
        assert_eq!(dy(40.0, 20.0, Align::Nearest), -60.0);
        assert_eq!(dy(260.0, 20.0, Align::Nearest), 80.0);
    }

    #[test]
    fn nearest_moves_a_partly_shown_node_the_least() {
        assert_eq!(dy(90.0, 20.0, Align::Nearest), -10.0);
        assert_eq!(dy(190.0, 20.0, Align::Nearest), 10.0);
    }

    #[test]
    fn nearest_aligns_the_start_of_a_node_longer_than_the_box() {
        assert_eq!(dy(150.0, 300.0, Align::Nearest), 50.0);
        assert_eq!(dy(60.0, 130.0, Align::Nearest), -40.0);
        assert_eq!(dy(20.0, 120.0, Align::Nearest), -80.0);
    }

    #[test]
    fn nearest_keeps_a_node_that_covers_the_box() {
        assert_eq!(dy(50.0, 300.0, Align::Nearest), 0.0);
    }

    #[test]
    fn reveal_keeps_a_node_that_shows() {
        assert_eq!(dy(130.0, 20.0, Align::Reveal), 0.0);
    }

    #[test]
    fn reveal_centres_a_node_outside_the_box() {
        assert_eq!(dy(40.0, 20.0, Align::Reveal), -100.0);
        assert_eq!(dy(260.0, 20.0, Align::Reveal), 120.0);
        assert_eq!(dy(200.0, 20.0, Align::Reveal), 60.0);
        assert_eq!(dy(80.0, 20.0, Align::Reveal), -60.0);
    }

    #[test]
    fn reveal_moves_a_partly_shown_node_the_least() {
        assert_eq!(dy(90.0, 20.0, Align::Reveal), -10.0);
        assert_eq!(dy(190.0, 20.0, Align::Reveal), 10.0);
        assert_eq!(dy(150.0, 300.0, Align::Reveal), 50.0);
    }

    #[test]
    fn center_outside_keeps_a_node_that_shows_fully_or_partly() {
        let center_outside = |top, height| dy(top, height, Align::CenterOutside);
        assert_eq!(center_outside(130.0, 20.0), 0.0);
        assert_eq!(center_outside(90.0, 20.0), 0.0);
        assert_eq!(center_outside(190.0, 20.0), 0.0);
        assert_eq!(center_outside(150.0, 300.0), 0.0);
    }

    #[test]
    fn center_outside_centres_a_node_outside_the_box() {
        assert_eq!(dy(40.0, 20.0, Align::CenterOutside), -100.0);
        assert_eq!(dy(260.0, 20.0, Align::CenterOutside), 120.0);
        assert_eq!(dy(200.0, 20.0, Align::CenterOutside), 60.0);
    }

    #[test]
    fn a_partly_shown_node_moves_on_the_block_axis_and_stays_on_the_inline_axis() {
        let (before, after) = (rect(90.0, 90.0, 20.0, 20.0), rect(190.0, 190.0, 20.0, 20.0));
        let delta = |node| scroll_delta(node, VIEW, Align::Reveal, Align::CenterOutside);
        assert_eq!(delta(before), (0.0, -10.0));
        assert_eq!(delta(after), (0.0, 10.0));
    }

    #[test]
    fn a_zero_size_node_inside_or_on_the_edge_of_the_box_shows() {
        for top in [100.0, 150.0, 200.0] {
            assert_eq!(dy(top, 0.0, Align::Reveal), 0.0);
            assert_eq!(dy(top, 0.0, Align::CenterOutside), 0.0);
        }
    }

    #[test]
    fn a_zero_size_node_outside_the_box_centres() {
        assert_eq!(dy(60.0, 0.0, Align::Reveal), -90.0);
        assert_eq!(dy(260.0, 0.0, Align::CenterOutside), 110.0);
    }

    #[test]
    fn a_box_of_zero_height_centres_a_node_outside_it() {
        let view = rect(100.0, 100.0, 100.0, 0.0);
        let node = rect(100.0, 140.0, 10.0, 20.0);
        assert_eq!(
            scroll_delta(node, view, Align::Reveal, Align::Reveal).1,
            50.0
        );
    }

    #[test]
    fn a_border_wider_than_half_the_box_leaves_a_zero_size_view() {
        let bounds = Bounds::new(point(px(10.0), px(10.0)), gpui::size(px(10.0), px(6.0)));
        let view = rect_of(bounds, px(8.0));
        assert_eq!((view.width, view.height), (0.0, 0.0));
        assert_eq!((view.x, view.y), (18.0, 18.0));
    }

    #[test]
    fn a_scroll_moves_the_recorded_boxes_below_the_container() {
        let mut host = Host::default();
        let container = host.root;
        let child = host.tree.create_node("div").unwrap();
        let grandchild = host.tree.create_node("div").unwrap();
        host.tree.append_child(container, child).unwrap();
        host.tree.append_child(child, grandchild).unwrap();
        let record = |id| {
            let layout = inca_gpui::NodeLayout {
                bounds: Bounds::new(point(px(10.0), px(50.0)), gpui::size(px(5.0), px(5.0))),
                border: px(0.0),
                scroll: point(px(1.0), px(2.0)),
            };
            host.layout.borrow_mut().insert(id, layout);
        };
        for id in [container, child, grandchild] {
            record(id);
        }

        shift_descendants(&host, container, 3.0, 20.0);

        let layout = host.layout.borrow();
        let read = |id| {
            let l = layout[&id];
            (
                f32::from(l.bounds.origin.x),
                f32::from(l.bounds.origin.y),
                f32::from(l.scroll.x),
                f32::from(l.scroll.y),
            )
        };
        assert_eq!(read(container), (10.0, 50.0, 1.0, 2.0));
        assert_eq!(read(child), (7.0, 30.0, 4.0, 22.0));
        assert_eq!(read(grandchild), (7.0, 30.0, 1.0, 2.0));
    }

    #[test]
    fn the_axes_take_their_own_alignment() {
        let node = rect(260.0, 40.0, 20.0, 20.0);
        let delta = scroll_delta(node, VIEW, Align::Start, Align::End);
        assert_eq!(delta, (80.0, -60.0));
    }

    #[test]
    fn the_inline_axis_is_horizontal() {
        let node = rect(260.0, 130.0, 20.0, 20.0);
        assert_eq!(
            scroll_delta(node, VIEW, Align::Nearest, Align::Nearest),
            (80.0, 0.0)
        );
    }

    #[test]
    fn a_container_sees_the_node_shifted_by_the_scroll_of_the_one_before() {
        let node = rect(0.0, 500.0, 10.0, 20.0);
        let chain = [
            (1, rect(0.0, 0.0, 50.0, 100.0)),
            (2, rect(0.0, 0.0, 50.0, 300.0)),
        ];
        let mut seen = Vec::new();
        scroll_chain(node, &chain, Align::Start, Align::Nearest, |id, dx, dy| {
            seen.push((id, dx, dy));
            // The inner container reaches only half of the way.
            (dx, if id == 1 { dy / 2.0 } else { dy })
        });
        assert_eq!(seen, [(1, 0.0, 500.0), (2, 0.0, 250.0)]);
    }
}
