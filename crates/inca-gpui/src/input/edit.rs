// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The edits of an input value and their event payloads.

use super::{InputState, InputType, Range, bytes_of_units};

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
    /// The `data` of the `beforeinput` of this edit.
    pub data: Option<String>,
}

/// The fields of `beforeinput` and `input`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputPayload {
    /// The inserted text, `None` for a deletion and a line break, and on `input` for an
    /// empty composition text.
    pub data: Option<String>,
    /// The kind of edit.
    pub input_type: InputType,
    /// A composition is active.
    pub is_composing: bool,
}

/// An edit that removes `range`.
fn removal(range: Range<usize>, input_type: InputType) -> Edit {
    Edit {
        input_type,
        selection: range.start..range.start,
        range,
        text: String::new(),
        composition: None,
        data: None,
    }
}

impl Edit {
    /// The fields of the `beforeinput` of this edit when `before` is true, else
    /// of its `input`.
    #[must_use]
    pub fn payload(&self, before: bool) -> InputPayload {
        let data = if before {
            self.data.clone()
        } else {
            match self.input_type {
                InputType::InsertText | InputType::InsertFromPaste => Some(self.text.clone()),
                InputType::InsertCompositionText if !self.text.is_empty() => {
                    Some(self.text.clone())
                }
                _ => None,
            }
        };
        InputPayload {
            data,
            input_type: self.input_type,
            is_composing: self.input_type == InputType::InsertCompositionText,
        }
    }
}

impl InputState {
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
            data: Some(text.to_owned()),
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
            data: Some(text.to_owned()),
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
        self.edited = true;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::input::tests::{at, caret_at};
    use rstest::rstest;

    #[test]
    fn insert_replaces_the_selection() {
        let edit = at("abcd", 1..3, false)
            .insert("x", InputType::InsertText)
            .unwrap();
        let want = Edit {
            input_type: InputType::InsertText,
            range: 1..3,
            text: "x".to_owned(),
            selection: 2..2,
            composition: None,
            data: Some("x".to_owned()),
        };
        assert_eq!(edit, want);
    }

    #[test]
    fn insert_replaces_the_composition() {
        let mut state = caret_at("abcd", 3);
        state.composition = Some(1..3);
        let edit = state.insert("x", InputType::InsertFromPaste).unwrap();
        assert_eq!(edit.range, 1..3);
        assert_eq!(edit.input_type, InputType::InsertFromPaste);
    }

    #[test]
    fn insert_of_nothing_over_nothing_is_no_edit() {
        assert_eq!(caret_at("ab", 1).insert("", InputType::InsertText), None);
    }

    #[test]
    fn replace_converts_its_utf16_range() {
        let edit = caret_at("a😀b", 0).replace(Some(1..3), "x").unwrap();
        assert_eq!(edit.range, 1..5);
        assert_eq!(edit.selection, 2..2);
        assert_eq!(edit.input_type, InputType::InsertText);
    }

    #[test]
    fn replace_while_composing_is_a_composition_edit() {
        let mut state = caret_at("abk", 3);
        state.composition = Some(2..3);
        let edit = state.replace(None, "か").unwrap();
        assert_eq!(edit.input_type, InputType::InsertCompositionText);
        assert_eq!(edit.range, 2..3);
        assert_eq!(edit.composition, None);
    }

    #[rstest]
    #[case::default_caret("ab", 1..2, None, "k", None, (1..2, 2..2, Some(1..2)))]
    #[case::selected("", 0..0, None, "か", Some(0..1), (0..0, 0..3, Some(0..3)))]
    #[case::empty_text("ab", 1..2, None, "", None, (1..2, 1..1, None))]
    #[case::explicit_range("a😀b", 0..0, Some(1..3), "k", None, (1..5, 2..2, Some(1..2)))]
    fn compose_marks_the_new_text(
        #[case] value: &str,
        #[case] selection: Range<usize>,
        #[case] range: Option<Range<usize>>,
        #[case] text: &str,
        #[case] selected: Option<Range<usize>>,
        #[case] want: (Range<usize>, Range<usize>, Option<Range<usize>>),
    ) {
        let edit = at(value, selection, false).compose(range, text, selected);
        assert_eq!(edit.input_type, InputType::InsertCompositionText);
        assert_eq!(edit.text, text);
        assert_eq!((edit.range, edit.selection, edit.composition), want);
    }

    #[rstest]
    #[case::back("abc", 2..2, false, 1..2, InputType::DeleteContentBackward)]
    #[case::back_combining("e\u{301}x", 3..3, false, 1..3, InputType::DeleteContentBackward)]
    #[case::forward_combining("e\u{301}x", 0..0, true, 0..3, InputType::DeleteContentForward)]
    #[case::selection("abcd", 1..3, false, 1..3, InputType::DeleteContentBackward)]
    fn delete_removes_the_range_of_its_direction(
        #[case] value: &str,
        #[case] selection: Range<usize>,
        #[case] forward: bool,
        #[case] want: Range<usize>,
        #[case] input_type: InputType,
    ) {
        let edit = at(value, selection, false).delete(forward);
        assert_eq!(edit.range, want);
        assert_eq!(edit.input_type, input_type);
        assert_eq!(edit.text, "");
        assert_eq!(edit.selection, want.start..want.start);
        assert_eq!(edit.composition, None);
    }

    #[rstest]
    #[case::back_at_start(0, false, 0..0)]
    #[case::forward_at_end(3, true, 3..3)]
    fn delete_at_an_edge_is_an_empty_edit(
        #[case] caret: usize,
        #[case] forward: bool,
        #[case] want: Range<usize>,
    ) {
        assert_eq!(caret_at("abc", caret).delete(forward).range, want);
    }

    #[test]
    fn cut_takes_the_selection() {
        let edit = at("abcd", 1..3, false).cut().unwrap();
        assert_eq!(edit.input_type, InputType::DeleteByCut);
        assert_eq!(edit.range, 1..3);
        assert_eq!(edit.selection, 1..1);
    }

    #[test]
    fn cut_of_an_empty_selection_is_no_edit() {
        assert!(at("abcd", 2..2, false).cut().is_none());
    }

    #[test]
    fn apply_writes_the_edit() {
        let mut state = at("abcd", 1..3, true);
        state.composition = Some(1..2);
        state.apply(&Edit {
            input_type: InputType::InsertCompositionText,
            range: 1..3,
            text: "xy".to_owned(),
            selection: 3..3,
            composition: Some(1..3),
            data: None,
        });
        assert_eq!(state.value(), "axyd");
        assert_eq!(state.selection(), 3..3);
        assert!(!state.reversed());
        assert_eq!(state.composition(), Some(1..3));
    }

    #[rstest]
    #[case::insert_before(InputType::InsertText, "x", Some("x"), true, Some("x"))]
    #[case::insert_after(InputType::InsertText, "x", Some("x"), false, Some("x"))]
    #[case::paste_before(InputType::InsertFromPaste, "x y", Some("x\ny"), true, Some("x\ny"))]
    #[case::paste_after(InputType::InsertFromPaste, "x y", Some("x\ny"), false, Some("x y"))]
    #[case::delete_before(InputType::DeleteContentBackward, "", None, true, None)]
    #[case::delete_after(InputType::DeleteContentBackward, "", None, false, None)]
    #[case::empty_composition_before(
        InputType::InsertCompositionText,
        "",
        Some(""),
        true,
        Some("")
    )]
    #[case::empty_composition_after(InputType::InsertCompositionText, "", Some(""), false, None)]
    fn an_edit_payload_reads_data_per_event(
        #[case] input_type: InputType,
        #[case] text: &str,
        #[case] data: Option<&str>,
        #[case] before: bool,
        #[case] want: Option<&str>,
    ) {
        let edit = Edit {
            input_type,
            range: 0..0,
            text: text.to_owned(),
            selection: 0..0,
            composition: None,
            data: data.map(str::to_owned),
        };
        let payload = edit.payload(before);
        assert_eq!(payload.data.as_deref(), want);
        assert_eq!(payload.input_type, input_type);
        assert_eq!(
            payload.is_composing,
            input_type == InputType::InsertCompositionText
        );
    }
}
