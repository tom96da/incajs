// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The geometry that scrolls a node into view.

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
    /// brings its near edge in. A node longer than the box aligns its end when
    /// its start lies before the box, and its start when its end lies after
    /// it. A node that covers the whole box stays put.
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
        let (before, after) = (node_start < view_start, node_end > view_end);
        let long = node_len > view_len;
        if before && after {
            0.0
        } else if (before && !long) || (after && long) {
            node_start - view_start
        } else if before || after {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;
    use rstest::rstest;

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
    fn nearest_aligns_the_start_of_a_longer_node_that_ends_after_the_box() {
        assert_eq!(dy(150.0, 300.0, Align::Nearest), 50.0);
        assert_eq!(dy(260.0, 300.0, Align::Nearest), 160.0);
    }

    #[test]
    fn nearest_aligns_the_end_of_a_longer_node_that_starts_before_the_box() {
        assert_eq!(dy(60.0, 130.0, Align::Nearest), -10.0);
        assert_eq!(dy(20.0, 120.0, Align::Nearest), -60.0);
        assert_eq!(dy(-50.0, 140.0, Align::Nearest), -110.0);
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
    fn reveal_centers_a_node_outside_the_box() {
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

    #[rstest]
    #[case::top_edge(100.0)]
    #[case::inside(150.0)]
    #[case::bottom_edge(200.0)]
    fn a_zero_size_node_inside_or_on_the_edge_of_the_box_shows(#[case] top: f32) {
        assert_eq!(dy(top, 0.0, Align::Reveal), 0.0);
        assert_eq!(dy(top, 0.0, Align::CenterOutside), 0.0);
    }

    #[test]
    fn a_zero_size_node_outside_the_box_centers() {
        assert_eq!(dy(60.0, 0.0, Align::Reveal), -90.0);
        assert_eq!(dy(260.0, 0.0, Align::CenterOutside), 110.0);
    }

    #[test]
    fn a_box_of_zero_height_centers_a_node_outside_it() {
        let view = rect(100.0, 100.0, 100.0, 0.0);
        let node = rect(100.0, 140.0, 10.0, 20.0);
        assert_eq!(
            scroll_delta(node, view, Align::Reveal, Align::Reveal).1,
            50.0
        );
    }

    #[test]
    fn the_axes_take_their_own_alignment() {
        let node = rect(260.0, 40.0, 20.0, 20.0);
        let delta = scroll_delta(node, VIEW, Align::Start, Align::End);
        assert_eq!(delta, (80.0, -60.0));
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
