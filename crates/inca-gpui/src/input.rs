// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where the last frame drew the text of an `input` node, and the platform's
//! text input calls. The text state itself is in `inca_document::text`.

use std::ops::Range;

use gpui::{Bounds, Pixels, point, px};

/// The width of the caret bar in pixels.
pub const CARET_WIDTH: f32 = 1.0;

/// A call of the platform's text input. Ranges are UTF-16 code units of the
/// value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputRequest {
    /// Replaces `range`, or the composition or selection without one, with
    /// `text`.
    Replace {
        range: Option<Range<usize>>,
        text: String,
    },
    /// Replaces as `Replace` does and marks `text` as the composition.
    /// `selected` is relative to `text`.
    Mark {
        range: Option<Range<usize>>,
        text: String,
        selected: Option<Range<usize>>,
    },
    /// Ends the composition and keeps its text.
    Unmark,
    /// Selects `range`.
    Select(Range<usize>),
}

/// Where the last frame drew the text of an `input`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputLayout {
    /// The window x of the text start, with the scroll applied.
    pub x: f32,
    /// The window y of the text start.
    pub y: f32,
    /// The height of the line box.
    pub height: f32,
    /// How far the text is shifted left.
    pub scroll: f32,
    /// Each char boundary of the drawn text with its x from the text start.
    pub stops: Vec<(usize, f32)>,
}

/// What a frame paints over the text: x positions from the text start.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Highlights {
    pub selection: Option<(f32, f32)>,
    pub caret: Option<f32>,
}

impl InputLayout {
    /// The offset of the recorded stop nearest to the window x `x`. The
    /// earlier stop wins a tie, and 0 answers without stops.
    #[must_use]
    pub fn offset_at(&self, x: f32) -> usize {
        let x = x - self.x;
        self.stops
            .iter()
            .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
            .map_or(0, |&(at, _)| at)
    }

    /// The x of `offset` from the text start.
    #[must_use]
    pub fn x_of(&self, offset: usize) -> f32 {
        x_at(&self.stops, offset)
    }

    /// The window box of `range` on the recorded line, `None` before a frame
    /// recorded a layout.
    #[must_use]
    pub fn bounds_of(&self, range: &Range<usize>) -> Option<Bounds<Pixels>> {
        if self.stops.is_empty() {
            return None;
        }
        Some(Bounds::from_corners(
            point(px(self.x + self.x_of(range.start)), px(self.y)),
            point(px(self.x + self.x_of(range.end)), px(self.y + self.height)),
        ))
    }
}

/// The x of the stop at `offset`, else of the last stop below it, else 0.
#[must_use]
pub fn x_at(stops: &[(usize, f32)], offset: usize) -> f32 {
    stops
        .iter()
        .take_while(|(at, _)| *at <= offset)
        .last()
        .map_or(0.0, |&(_, x)| x)
}

/// The stop of every char boundary of `value`, the end included, with the x
/// that `x_for` gives it.
#[must_use]
pub fn stops_of(value: &str, x_for: impl Fn(usize) -> f32) -> Vec<(usize, f32)> {
    let mut stops: Vec<_> = value
        .char_indices()
        .map(|(at, _)| (at, x_for(at)))
        .collect();
    stops.push((value.len(), x_for(value.len())));
    stops
}

/// The scroll that keeps the caret inside a box `width` wide, starting from
/// `scroll`, for a text `line_width` wide.
#[must_use]
pub fn scroll_to_caret(scroll: f32, caret: f32, line_width: f32, width: f32) -> f32 {
    let room = (width - CARET_WIDTH).max(0.0);
    let scroll = if caret < scroll {
        caret
    } else if caret > scroll + room {
        caret - room
    } else {
        scroll
    };
    scroll.clamp(0.0, (line_width + CARET_WIDTH - width).max(0.0))
}

/// What to paint for `selection` and `caret`. Nothing paints while the node
/// is not `focused`.
#[must_use]
pub fn highlights(
    stops: &[(usize, f32)],
    selection: &Range<usize>,
    caret: usize,
    focused: bool,
) -> Highlights {
    if !focused {
        return Highlights::default();
    }
    if selection.is_empty() {
        Highlights {
            selection: None,
            caret: Some(x_at(stops, caret)),
        }
    } else {
        Highlights {
            selection: Some((x_at(stops, selection.start), x_at(stops, selection.end))),
            caret: None,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use rstest::rstest;

    /// A layout of a line starting at window x `x` with `stops`.
    pub(crate) fn laid_out(x: f32, stops: &[(usize, f32)]) -> InputLayout {
        InputLayout {
            x,
            y: 20.0,
            height: 16.0,
            scroll: 0.0,
            stops: stops.to_vec(),
        }
    }

    #[rstest]
    #[case::exact(&[(0, 0.0), (3, 12.0), (6, 24.0)], 3, 12.0)]
    #[case::between(&[(0, 0.0), (3, 12.0), (6, 24.0)], 4, 12.0)]
    #[case::empty(&[], 3, 0.0)]
    fn x_at_reads_the_stop_of_an_offset(
        #[case] stops: &[(usize, f32)],
        #[case] offset: usize,
        #[case] want: f32,
    ) {
        assert_eq!(x_at(stops, offset), want);
    }

    #[rstest]
    #[case::ascii("ab", &[0, 1, 2])]
    #[case::wide("é😀", &[0, 2, 6])]
    #[case::empty("", &[0])]
    fn stops_of_lists_every_char_boundary(#[case] value: &str, #[case] offsets: &[usize]) {
        #[allow(clippy::cast_precision_loss)]
        let x_for = |at: usize| at as f32 * 2.0;
        let want: Vec<_> = offsets.iter().map(|&at| (at, x_for(at))).collect();
        assert_eq!(stops_of(value, x_for), want);
    }

    #[rstest]
    #[case::inside(0.0, 10.0, 50.0, 30.0, 0.0)]
    #[case::right(0.0, 40.0, 60.0, 30.0, 11.0)]
    #[case::left(20.0, 5.0, 60.0, 30.0, 5.0)]
    #[case::short_line(50.0, 10.0, 10.0, 30.0, 0.0)]
    #[case::end(0.0, 60.0, 60.0, 30.0, 31.0)]
    fn scroll_to_caret_keeps_the_caret_in_the_box(
        #[case] scroll: f32,
        #[case] caret: f32,
        #[case] line_width: f32,
        #[case] width: f32,
        #[case] want: f32,
    ) {
        assert_eq!(scroll_to_caret(scroll, caret, line_width, width), want);
    }

    #[rstest]
    #[case::unfocused(0..2, 2, false, None, None)]
    #[case::unfocused_caret(1..1, 1, false, None, None)]
    #[case::caret(1..1, 1, true, None, Some(6.0))]
    #[case::selection(0..2, 2, true, Some((0.0, 12.0)), None)]
    fn highlights_show_only_while_focused(
        #[case] selection: Range<usize>,
        #[case] caret: usize,
        #[case] focused: bool,
        #[case] want_selection: Option<(f32, f32)>,
        #[case] want_caret: Option<f32>,
    ) {
        let stops = [(0, 0.0), (1, 6.0), (2, 12.0)];
        let want = Highlights {
            selection: want_selection,
            caret: want_caret,
        };
        assert_eq!(highlights(&stops, &selection, caret, focused), want);
    }

    #[rstest]
    #[case::nearer_right(0.0, 13.0, 2)]
    #[case::tie(0.0, 3.0, 0)]
    #[case::left_of_text(0.0, -5.0, 0)]
    #[case::right_of_text(0.0, 99.0, 3)]
    #[case::scrolled(-6.0, 0.0, 1)]
    fn offset_at_finds_the_nearest_stop(
        #[case] layout_x: f32,
        #[case] x: f32,
        #[case] want: usize,
    ) {
        let stops = [(0, 0.0), (1, 6.0), (2, 12.0), (3, 18.0)];
        assert_eq!(laid_out(layout_x, &stops).offset_at(x), want);
    }

    #[test]
    fn offset_at_gives_0_without_a_layout() {
        assert_eq!(InputLayout::default().offset_at(13.0), 0);
    }

    #[rstest]
    #[case::recorded(
        &[(0, 0.0), (1, 6.0), (2, 12.0)],
        Some(Bounds::from_corners(point(px(16.), px(20.)), point(px(22.), px(36.))))
    )]
    #[case::unrecorded(&[], None)]
    fn bounds_of_reads_the_recorded_layout(
        #[case] stops: &[(usize, f32)],
        #[case] want: Option<Bounds<Pixels>>,
    ) {
        assert_eq!(laid_out(10.0, stops).bounds_of(&(1..2)), want);
    }
}
