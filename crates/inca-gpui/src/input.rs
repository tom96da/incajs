// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text, caret, selection and composition of an `input` node, in byte
//! offsets of its value on char boundaries, and the edits that change them.
//! The platform's text input speaks UTF-16 code units; the conversions are
//! here.

use std::ops::Range;

use gpui::{Bounds, Pixels, point, px};
use unicode_segmentation::UnicodeSegmentation;

/// The width of the caret bar in pixels.
pub const CARET_WIDTH: f32 = 1.0;

/// The `inputType` of an edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    InsertText,
    InsertFromPaste,
    InsertCompositionText,
    /// A line break (Enter).
    InsertLineBreak,
    DeleteContentBackward,
    DeleteContentForward,
    DeleteByCut,
}

impl InputType {
    /// The `inputType` string of the variant.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InsertText => "insertText",
            Self::InsertFromPaste => "insertFromPaste",
            Self::InsertCompositionText => "insertCompositionText",
            Self::InsertLineBreak => "insertLineBreak",
            Self::DeleteContentBackward => "deleteContentBackward",
            Self::DeleteContentForward => "deleteContentForward",
            Self::DeleteByCut => "deleteByCut",
        }
    }
}

/// A caret move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Back,
    Forward,
    Start,
    End,
}

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

/// A change to the value and the state after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub input_type: InputType,
    /// The bytes of the value that `text` replaces.
    pub range: Range<usize>,
    pub text: String,
    /// The selection after the edit.
    pub selection: Range<usize>,
    /// The composition after the edit.
    pub composition: Option<Range<usize>>,
}

/// What a frame paints over the text: x positions from the text start.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Highlights {
    pub selection: Option<(f32, f32)>,
    pub caret: Option<f32>,
}

/// The text state of one `input` node.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputState {
    value: String,
    selection: Range<usize>,
    reversed: bool,
    composition: Option<Range<usize>>,
    layout: InputLayout,
}

/// `text` without the chars for which [`char::is_control`] holds.
#[must_use]
pub fn typed(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// `text` without CR and LF.
#[must_use]
pub fn sanitized(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\r' | '\n')).collect()
}

/// `text` with each CRLF, then each remaining CR and LF, replaced by one
/// space.
#[must_use]
pub fn pasted(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
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

/// The first char boundary of `text` whose UTF-16 count reaches `units`. A
/// unit inside a surrogate pair rounds up, and a count past the end gives
/// the length.
fn bytes_of_units(text: &str, units: usize) -> usize {
    let mut count = 0;
    for (at, c) in text.char_indices() {
        if count >= units {
            return at;
        }
        count += c.len_utf16();
    }
    text.len()
}

/// An edit that removes `range`.
fn removal(range: Range<usize>, input_type: InputType) -> Edit {
    Edit {
        input_type,
        selection: range.start..range.start,
        range,
        text: String::new(),
        composition: None,
    }
}

impl InputState {
    /// The text.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The selected bytes of the text.
    #[must_use]
    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    /// Whether the caret sits at the start of the selection.
    #[must_use]
    pub fn reversed(&self) -> bool {
        self.reversed
    }

    /// The end of the selection that moves.
    #[must_use]
    pub fn caret(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    /// The end of the selection that stays.
    #[must_use]
    pub fn anchor(&self) -> usize {
        if self.reversed {
            self.selection.end
        } else {
            self.selection.start
        }
    }

    /// The composed bytes of the text.
    #[must_use]
    pub fn composition(&self) -> Option<Range<usize>> {
        self.composition.clone()
    }

    /// The selected text.
    #[must_use]
    pub fn selected_text(&self) -> &str {
        &self.value[self.selection.clone()]
    }

    /// The layout the last frame recorded.
    #[must_use]
    pub fn layout(&self) -> &InputLayout {
        &self.layout
    }

    /// The nearest char boundary at or below `offset`, at most the length.
    fn clamp(&self, offset: usize) -> usize {
        self.value.floor_char_boundary(offset)
    }

    /// Replaces the text with `value` without CR and LF. A changed text puts
    /// the caret at its end and ends the composition. Returns whether the
    /// text changed.
    pub fn set_value(&mut self, value: &str) -> bool {
        let value = sanitized(value);
        if value == self.value {
            return false;
        }
        let end = value.len();
        self.value = value;
        self.selection = end..end;
        self.reversed = false;
        self.composition = None;
        true
    }

    /// Selects from `anchor` to `focus`. The caret sits at `focus`.
    pub fn select(&mut self, anchor: usize, focus: usize) {
        let (anchor, focus) = (self.clamp(anchor), self.clamp(focus));
        self.selection = anchor.min(focus)..anchor.max(focus);
        self.reversed = focus < anchor;
    }

    /// Selects the whole text.
    pub fn select_all(&mut self) {
        self.select(0, self.value.len());
    }

    /// The start of the grapheme after `from` going `forward`, else before
    /// it, else the end of the text or 0.
    #[must_use]
    pub fn step(&self, from: usize, forward: bool) -> usize {
        let mut starts = self.value.grapheme_indices(true).map(|(at, _)| at);
        if forward {
            starts.find(|&at| at > from).unwrap_or(self.value.len())
        } else {
            starts.take_while(|&at| at < from).last().unwrap_or(0)
        }
    }

    /// Moves the caret, and the selection end with it while `extend` holds.
    /// `macos` makes Shift+Home and Shift+End extend the selection to the
    /// target and keep its far end.
    pub fn motion(&mut self, motion: Motion, extend: bool, macos: bool) {
        let caret = self.caret();
        let collapses = !extend && !self.selection.is_empty();
        let target = match motion {
            Motion::Back if collapses => self.selection.start,
            Motion::Back => self.step(caret, false),
            Motion::Forward if collapses => self.selection.end,
            Motion::Forward => self.step(caret, true),
            Motion::Start => 0,
            Motion::End => self.value.len(),
        };
        if !extend {
            self.select(target, target);
        } else if macos && matches!(motion, Motion::Start | Motion::End) {
            let reversed = if self.selection.is_empty() {
                target < caret
            } else {
                self.reversed
            };
            self.selection = self.selection.start.min(target)..self.selection.end.max(target);
            self.reversed = reversed;
        } else {
            self.select(self.anchor(), target);
        }
    }

    /// Puts the caret at `offset`, and keeps the anchor while `extend` holds.
    pub fn place(&mut self, offset: usize, extend: bool) {
        if extend {
            self.select(self.anchor(), offset);
        } else {
            self.select(offset, offset);
        }
    }

    /// The offset of the recorded stop nearest to the window x `x`. The
    /// earlier stop wins a tie, and 0 answers without stops.
    #[must_use]
    pub fn offset_at(&self, x: f32) -> usize {
        let x = x - self.layout.x;
        self.layout
            .stops
            .iter()
            .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
            .map_or(0, |&(at, _)| at)
    }

    /// The x of `offset` from the text start.
    #[must_use]
    pub fn x_of(&self, offset: usize) -> f32 {
        x_at(&self.layout.stops, offset)
    }

    /// The window box of `range` on the recorded line, `None` before a frame
    /// recorded a layout.
    #[must_use]
    pub fn bounds_of(&self, range: &Range<usize>) -> Option<Bounds<Pixels>> {
        if self.layout.stops.is_empty() {
            return None;
        }
        let layout = &self.layout;
        Some(Bounds::from_corners(
            point(px(layout.x + self.x_of(range.start)), px(layout.y)),
            point(
                px(layout.x + self.x_of(range.end)),
                px(layout.y + layout.height),
            ),
        ))
    }

    /// The UTF-16 offset of the byte `offset`.
    #[must_use]
    pub fn to_utf16(&self, offset: usize) -> usize {
        self.value[..self.clamp(offset)].encode_utf16().count()
    }

    /// The byte offset of the UTF-16 offset `units`.
    #[must_use]
    pub fn from_utf16(&self, units: usize) -> usize {
        bytes_of_units(&self.value, units)
    }

    /// `range` in UTF-16 offsets.
    #[must_use]
    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.to_utf16(range.start)..self.to_utf16(range.end)
    }

    /// `range` in byte offsets.
    #[must_use]
    pub fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.from_utf16(range.start)..self.from_utf16(range.end)
    }

    /// The length of the text in UTF-16 code units.
    #[must_use]
    pub fn len_utf16(&self) -> usize {
        self.value.encode_utf16().count()
    }

    /// The composition, else the selection.
    fn target(&self) -> Range<usize> {
        self.composition
            .clone()
            .unwrap_or_else(|| self.selection.clone())
    }

    /// An edit that puts `text` over `range` and the caret after it, `None`
    /// when both are empty.
    fn replacement(range: Range<usize>, text: &str, input_type: InputType) -> Option<Edit> {
        if text.is_empty() && range.is_empty() {
            return None;
        }
        let end = range.start + text.len();
        Some(Edit {
            input_type,
            range,
            text: text.to_owned(),
            selection: end..end,
            composition: None,
        })
    }

    /// The edit that puts `text` over the composition, else over the
    /// selection. `None` when both are empty.
    #[must_use]
    pub fn insert(&self, text: &str, input_type: InputType) -> Option<Edit> {
        Self::replacement(self.target(), text, input_type)
    }

    /// The edit for a platform replacement of `range` (UTF-16 units), else of
    /// the composition or selection. It is a composition edit while a
    /// composition is active.
    #[must_use]
    pub fn replace(&self, range: Option<Range<usize>>, text: &str) -> Option<Edit> {
        let range = range.map_or_else(|| self.target(), |range| self.range_from_utf16(&range));
        let input_type = if self.composition.is_some() {
            InputType::InsertCompositionText
        } else {
            InputType::InsertText
        };
        Self::replacement(range, text, input_type)
    }

    /// The edit that puts `text` over `range` (UTF-16 units), else over the
    /// composition or selection, and marks it as the composition. `selected`
    /// (UTF-16 units of `text`) is the selection after the edit, the end of
    /// `text` without it.
    #[must_use]
    pub fn compose(
        &self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) -> Edit {
        let range = range.map_or_else(|| self.target(), |range| self.range_from_utf16(&range));
        let start = range.start;
        let end = start + text.len();
        let selection = selected.map_or(end..end, |selected| {
            start + bytes_of_units(text, selected.start)..start + bytes_of_units(text, selected.end)
        });
        Edit {
            input_type: InputType::InsertCompositionText,
            range,
            text: text.to_owned(),
            selection,
            composition: (!text.is_empty()).then_some(start..end),
        }
    }

    /// The edit for Backspace or Delete: the selection, else the char before
    /// the caret, or the grapheme after it going `forward`. At an edge the
    /// range is empty.
    #[must_use]
    pub fn delete(&self, forward: bool) -> Edit {
        let input_type = if forward {
            InputType::DeleteContentForward
        } else {
            InputType::DeleteContentBackward
        };
        let caret = self.caret();
        let range = if !self.selection.is_empty() {
            self.selection.clone()
        } else if forward {
            caret..self.step(caret, true)
        } else {
            self.clamp(caret.saturating_sub(1))..caret
        };
        removal(range, input_type)
    }

    /// The edit that cuts the selection, `None` for an empty one.
    #[must_use]
    pub fn cut(&self) -> Option<Edit> {
        (!self.selection.is_empty())
            .then(|| removal(self.selection.clone(), InputType::DeleteByCut))
    }

    /// Writes `edit` into the text, the selection and the composition.
    pub fn apply(&mut self, edit: &Edit) {
        self.value.replace_range(edit.range.clone(), &edit.text);
        self.selection = edit.selection.clone();
        self.reversed = false;
        self.composition.clone_from(&edit.composition);
    }

    /// Ends the composition and keeps its text.
    pub fn unmark(&mut self) {
        self.composition = None;
    }

    /// Stores the layout the frame being drawn gave the text.
    pub fn record(&mut self, layout: InputLayout) {
        self.layout = layout;
    }
}

#[cfg(test)]
mod tests;
