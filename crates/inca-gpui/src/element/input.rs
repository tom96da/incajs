// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text line of an `input` node and its handler for the platform's text
//! input.

use std::ops::Range;

use gpui::{
    App, Bounds, ContentMask, Element, ElementId, FocusHandle, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, ShapedLine, SharedString, Style,
    TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window, fill, point, px, relative, rgba,
    size,
};

use crate::event_sink::EventSink;
use crate::input::{
    CARET_WIDTH, Highlights, InputLayout, InputRequest, InputState, highlights, scroll_to_caret,
    stops_of, x_at,
};
use crate::tree::NodeId;

/// The selection fill.
const SELECTION: u32 = 0x3390_ff66;

/// Answers the platform's text input for the `input` node `node` through
/// `sink`.
pub struct TextInputHandler<E> {
    sink: E,
    node: NodeId,
    editable: bool,
}

impl<E> TextInputHandler<E> {
    /// A handler for `node`. `editable` is its answer to `accepts_text_input`
    /// and `prefers_ime_for_printable_keys`.
    pub fn new(sink: E, node: NodeId, editable: bool) -> Self {
        Self {
            sink,
            node,
            editable,
        }
    }
}

impl<E: EventSink> TextInputHandler<E> {
    fn state(&self) -> Option<InputState> {
        self.sink.input(self.node)
    }

    fn request(&self, request: InputRequest, window: &mut Window, cx: &mut App) {
        self.sink.input_request(self.node, request, window, cx);
    }
}

impl<E: EventSink + 'static> gpui::InputHandler for TextInputHandler<E> {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<UTF16Selection> {
        let state = self.state()?;
        Some(UTF16Selection {
            range: state.range_to_utf16(&state.selection()),
            reversed: state.reversed(),
        })
    }

    fn marked_text_range(&mut self, _window: &mut Window, _cx: &mut App) -> Option<Range<usize>> {
        let state = self.state()?;
        state
            .composition()
            .map(|range| state.range_to_utf16(&range))
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<String> {
        let state = self.state()?;
        let range = state.range_from_utf16(&range_utf16);
        *adjusted_range = Some(state.range_to_utf16(&range));
        Some(state.value()[range].to_owned())
    }

    fn replace_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        let request = InputRequest::Replace {
            range: replacement_range,
            text: text.to_owned(),
        };
        self.request(request, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let request = InputRequest::Mark {
            range: range_utf16,
            text: new_text.to_owned(),
            selected: new_selected_range,
        };
        self.request(request, window, cx);
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut App) {
        self.request(InputRequest::Unmark, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        let state = self.state()?;
        state.bounds_of(&state.range_from_utf16(&range_utf16))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        let state = self.state()?;
        Some(state.to_utf16(state.offset_at(f32::from(point.x))))
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.request(InputRequest::Select(range_utf16), window, cx);
    }

    fn text_length_utf16(&mut self, _window: &mut Window, _cx: &mut App) -> Option<usize> {
        self.state().map(|state| state.len_utf16())
    }

    fn accepts_text_input(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        self.editable
    }

    fn prefers_ime_for_printable_keys(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        self.editable
    }
}

/// The element that draws the text of an `input` node over its box.
pub(super) struct InputElement<E> {
    node: NodeId,
    placeholder: SharedString,
    editable: bool,
    sink: Option<E>,
}

impl<E> InputElement<E> {
    pub(super) fn new(
        node: NodeId,
        placeholder: SharedString,
        editable: bool,
        sink: Option<E>,
    ) -> Self {
        Self {
            node,
            placeholder,
            editable,
            sink,
        }
    }
}

/// What `prepaint` hands to `paint`.
pub(super) struct Prepainted {
    line: ShapedLine,
    origin: Point<Pixels>,
    color: Hsla,
    highlights: Highlights,
    layout: InputLayout,
    focus: Option<FocusHandle>,
}

impl<E: EventSink + Clone + 'static> IntoElement for InputElement<E> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<E: EventSink + Clone + 'static> Element for InputElement<E> {
    type RequestLayoutState = ();
    type PrepaintState = Prepainted;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let state = self
            .sink
            .as_ref()
            .and_then(|sink| sink.input(self.node))
            .unwrap_or_default();
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let color = style.color;
        let placeholder = state.value().is_empty();
        let (text, text_color) = if placeholder {
            (self.placeholder.clone(), color.opacity(0.5))
        } else {
            (SharedString::from(state.value().to_owned()), color)
        };
        let mut run = style.to_run(text.len());
        run.color = text_color;
        let runs = text_runs(text.len(), state.composition(), &run);
        let line = window
            .text_system()
            .shape_line(text, font_size, &runs, None);
        let stops = if placeholder {
            vec![(0, 0.0)]
        } else {
            stops_of(state.value(), |at| f32::from(line.x_for_index(at)))
        };
        let focus = self
            .sink
            .as_ref()
            .and_then(|sink| sink.focus_handle(self.node));
        let focused = focus
            .as_ref()
            .is_some_and(|handle| handle.is_focused(window));
        let scroll = scroll_to_caret(
            state.layout().scroll,
            x_at(&stops, state.caret()),
            f32::from(line.width()),
            f32::from(bounds.size.width),
        );
        let origin = point(bounds.left() - px(scroll), bounds.top());
        let highlights = highlights(&stops, &state.selection(), state.caret(), focused);
        let layout = InputLayout {
            x: f32::from(origin.x),
            y: f32::from(bounds.top()),
            height: f32::from(bounds.size.height),
            scroll,
            stops,
        };
        Prepainted {
            line,
            origin,
            color,
            highlights,
            layout,
            focus,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let (Some(sink), Some(focus)) = (&self.sink, &prepaint.focus) {
            let handler = TextInputHandler::new(sink.clone(), self.node, self.editable);
            window.handle_input(focus, handler, cx);
        }
        let origin = prepaint.origin;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if let Some((from, to)) = prepaint.highlights.selection {
                let span = Bounds::from_corners(
                    point(origin.x + px(from), bounds.top()),
                    point(origin.x + px(to), bounds.bottom()),
                );
                window.paint_quad(fill(span, rgba(SELECTION)));
            }
            prepaint
                .line
                .paint(
                    origin,
                    window.line_height(),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            if let Some(at) = prepaint.highlights.caret {
                let bar = Bounds::new(
                    point(origin.x + px(at), bounds.top()),
                    size(px(CARET_WIDTH), bounds.size.height),
                );
                window.paint_quad(fill(bar, prepaint.color));
            }
        });
        if let Some(sink) = &self.sink {
            sink.record_input(self.node, std::mem::take(&mut prepaint.layout));
        }
    }
}

/// The runs of a line of `len` bytes: `run` over the whole line, or with a
/// `composition`, the runs before, inside and after it, the inside one
/// underlined. Runs of length 0 are dropped.
pub(super) fn text_runs(
    len: usize,
    composition: Option<Range<usize>>,
    run: &TextRun,
) -> Vec<TextRun> {
    let Some(composition) = composition else {
        return vec![TextRun { len, ..run.clone() }];
    };
    let underline = UnderlineStyle {
        thickness: px(1.),
        color: Some(run.color),
        wavy: false,
    };
    [
        (composition.start, None),
        (composition.end - composition.start, Some(underline)),
        (len - composition.end, None),
    ]
    .into_iter()
    .filter(|(len, _)| *len > 0)
    .map(|(len, underline)| TextRun {
        len,
        underline,
        ..run.clone()
    })
    .collect()
}
