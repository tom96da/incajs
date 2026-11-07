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
use inca_node::tree::NodeId;

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

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::element::input::text_runs;
    use crate::tests::*;
    use crate::{render_tree, render_tree_with_events};
    use gpui::InputHandler as _;
    use gpui::{
        Context, Keystroke, Render, TestAppContext, TextStyle, VisualTestContext, point, size,
    };
    use inca_node::VirtualTree;
    use inca_test_macros::gpui_cases;
    use rstest::rstest;
    use std::ops::Range;

    /// An `input` root 100 px wide at text size 10: 6 px per char, 16 px tall.
    fn input_tree(width: f64) -> VirtualTree {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("input").unwrap();
        tree.set_style(id, "width", width).unwrap();
        tree.set_style(id, "text_size", 10.0).unwrap();
        tree
    }

    /// A probe holding `value` as the text state of node 0.
    fn probe_with(tree: VirtualTree, value: &str) -> Probe {
        let probe = probe_of(tree);
        let mut state = InputState::default();
        state.set_value(value);
        probe.inputs.borrow_mut().insert(0, state);
        probe
    }

    fn drawn(cx: &mut VisualTestContext, probe: &Probe) {
        cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
            render_tree_with_events(&probe.tree.borrow(), 0, probe).unwrap()
        });
    }

    fn state_of(probe: &Probe) -> InputState {
        probe.inputs.borrow()[&0].clone()
    }

    struct PlainView(VirtualTree);

    impl Render for PlainView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            render_tree(&self.0, 0).unwrap()
        }
    }

    #[gpui::test]
    fn an_input_without_a_sink_registers_no_input_handler(cx: &mut TestAppContext) {
        let mut tree = input_tree(100.0);
        tree.set_attribute(0, "placeholder", "hint").unwrap();
        let window = cx.add_window(|_, _| PlainView(tree));
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let handled =
            cx.update(|window, cx| window.dispatch_keystroke(Keystroke::parse("x").unwrap(), cx));
        assert!(!handled);
    }

    #[gpui::test]
    fn an_input_records_a_caret_stop_per_char(cx: &mut TestAppContext) {
        let probe = probe_with(input_tree(100.0), "ab");
        drawn(cx.add_empty_window(), &probe);
        let state = probe.inputs.borrow()[&0].clone();
        let layout = state.layout();
        assert_eq!(layout.stops, [(0, 0.), (1, 6.), (2, 12.)]);
        assert_eq!(layout.x, 0.);
        assert_eq!(layout.height, 16.);
    }

    #[gpui::test]
    fn an_empty_input_records_one_stop(cx: &mut TestAppContext) {
        let mut tree = input_tree(100.0);
        tree.set_attribute(0, "placeholder", "hint").unwrap();
        let probe = probe_of(tree);
        drawn(cx.add_empty_window(), &probe);
        assert_eq!(state_of(&probe).layout().stops, [(0, 0.)]);
    }

    #[gpui::test]
    fn an_input_shifts_long_text_to_show_the_caret(cx: &mut TestAppContext) {
        let probe = probe_with(input_tree(30.0), "abcdefghij");
        drawn(cx.add_empty_window(), &probe);
        let state = state_of(&probe);
        assert_eq!(state.layout().scroll, 31.);
        assert_eq!(state.layout().x, -31.);
    }

    #[gpui::test]
    fn a_focused_input_takes_typed_text(cx: &mut TestAppContext) {
        let probe = probe_with(input_tree(100.0), "");
        let view = probe.clone();
        let window = cx.add_window(|_, _| ProbeView(view));
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let handle = cx.update(|_, cx| cx.focus_handle());
        *probe.focus.borrow_mut() = Some((0, handle.clone()));
        cx.update(|window, cx| {
            handle.focus(window, cx);
            window.draw(cx).clear(cx);
        });
        cx.simulate_input("x");
        let want = InputRequest::Replace {
            range: None,
            text: "x".to_owned(),
        };
        assert_eq!(*probe.requests.borrow(), [(0, want)]);
    }

    /// The handler of node 0 over a probe whose state is `state`.
    fn handler_over(state: Option<InputState>, editable: bool) -> (Probe, TextInputHandler<Probe>) {
        let probe = probe();
        if let Some(state) = state {
            probe.inputs.borrow_mut().insert(0, state);
        }
        let handler = TextInputHandler::new(probe.clone(), 0, editable);
        (probe, handler)
    }

    fn state_with(value: &str, selection: Range<usize>, reversed: bool) -> InputState {
        let mut state = InputState::default();
        state.set_value(value);
        if reversed {
            state.select(selection.end, selection.start);
        } else {
            state.select(selection.start, selection.end);
        }
        state
    }

    #[gpui::test]
    fn the_handler_reports_the_selection_in_utf16(cx: &mut TestAppContext) {
        let (_, mut handler) = handler_over(Some(state_with("a😀b", 1..5, true)), true);
        let got = cx
            .add_empty_window()
            .update(|window, cx| handler.selected_text_range(false, window, cx))
            .unwrap();
        assert_eq!(got.range, 1..3);
        assert!(got.reversed);
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::composing(&["かな"], Some(0..2))]
    #[case::idle(&[], None)]
    fn the_handler_reports_the_composition_in_utf16(
        cx: &mut TestAppContext,
        #[case] marks: &[&str],
        #[case] want: Option<Range<usize>>,
    ) {
        let mut state = InputState::default();
        for text in marks {
            state.apply(&state.compose(None, text, None));
        }
        let (_, mut handler) = handler_over(Some(state), true);
        let got = cx
            .add_empty_window()
            .update(|window, cx| handler.marked_text_range(window, cx));
        assert_eq!(got, want);
    }

    #[gpui::test]
    fn the_handler_returns_the_text_of_an_adjusted_range(cx: &mut TestAppContext) {
        let (_, mut handler) = handler_over(Some(state_with("a😀", 0..0, false)), true);
        let mut adjusted = None;
        let got = cx
            .add_empty_window()
            .update(|window, cx| handler.text_for_range(1..2, &mut adjusted, window, cx));
        assert_eq!(got.as_deref(), Some("😀"));
        assert_eq!(adjusted, Some(1..3));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::replace(
        |h, w, cx| h.replace_text_in_range(Some(1..2), "x", w, cx),
        InputRequest::Replace { range: Some(1..2), text: "x".to_owned() }
    )]
    #[case::mark(
        |h, w, cx| h.replace_and_mark_text_in_range(None, "k", Some(0..1), w, cx),
        InputRequest::Mark { range: None, text: "k".to_owned(), selected: Some(0..1) }
    )]
    #[case::unmark(|h, w, cx| h.unmark_text(w, cx), InputRequest::Unmark)]
    #[case::select(|h, w, cx| h.set_selected_text_range(0..1, w, cx), InputRequest::Select(0..1))]
    fn the_handler_forwards_each_write(
        cx: &mut TestAppContext,
        #[case] call: fn(&mut TextInputHandler<Probe>, &mut Window, &mut App),
        #[case] request: InputRequest,
    ) {
        let (probe, mut handler) = handler_over(None, true);
        cx.add_empty_window()
            .update(|window, cx| call(&mut handler, window, cx));
        assert_eq!(*probe.requests.borrow(), [(0, request)]);
    }

    /// A state that recorded a line at window x 10, y 20, 6 px per char.
    fn recorded(value: &str) -> InputState {
        let mut state = state_with(value, 0..0, false);
        let stops = crate::input::stops_of(value, |at| f32::from(u16::try_from(at).unwrap()) * 6.0);
        state.record(InputLayout {
            x: 10.0,
            y: 20.0,
            height: 16.0,
            scroll: 0.0,
            stops,
        });
        state
    }

    #[gpui::test]
    fn the_handler_reports_the_box_of_a_range(cx: &mut TestAppContext) {
        let (_, mut handler) = handler_over(Some(recorded("abc")), true);
        let got = cx
            .add_empty_window()
            .update(|window, cx| handler.bounds_for_range(1..2, window, cx));
        let want = gpui::Bounds::from_corners(point(px(16.), px(20.)), point(px(22.), px(36.)));
        assert_eq!(got, Some(want));
    }

    #[gpui::test]
    fn the_handler_maps_a_point_to_a_utf16_offset(cx: &mut TestAppContext) {
        let (_, mut handler) = handler_over(Some(recorded("a😀")), true);
        let got = cx.add_empty_window().update(|window, cx| {
            handler.character_index_for_point(point(px(19.), px(99.)), window, cx)
        });
        assert_eq!(got, Some(1));
    }

    #[gpui::test]
    fn the_handler_reports_the_length_in_utf16(cx: &mut TestAppContext) {
        let (_, mut handler) = handler_over(Some(state_with("a😀", 0..0, false)), true);
        let got = cx
            .add_empty_window()
            .update(|window, cx| handler.text_length_utf16(window, cx));
        assert_eq!(got, Some(3));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::editable(true)]
    #[case::readonly(false)]
    fn the_handler_accepts_text_only_when_editable(
        cx: &mut TestAppContext,
        #[case] editable: bool,
    ) {
        let (_, mut handler) = handler_over(None, editable);
        let got = cx
            .add_empty_window()
            .update(|window, cx| handler.accepts_text_input(window, cx));
        assert_eq!(got, editable);
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::editable(true)]
    #[case::readonly(false)]
    fn the_handler_prefers_ime_only_when_editable(cx: &mut TestAppContext, #[case] editable: bool) {
        let (_, mut handler) = handler_over(None, editable);
        let got = cx
            .add_empty_window()
            .update(|window, cx| handler.prefers_ime_for_printable_keys(window, cx));
        assert_eq!(got, editable);
    }

    #[gpui::test]
    fn the_handler_of_an_unknown_node_reports_nothing(cx: &mut TestAppContext) {
        let (_, mut handler) = handler_over(None, true);
        let got = cx
            .add_empty_window()
            .update(|window, cx| handler.selected_text_range(false, window, cx));
        assert!(got.is_none());
    }

    #[rstest]
    #[case::none(None, &[(5, false)])]
    #[case::middle(Some(1..3), &[(1, false), (2, true), (2, false)])]
    #[case::at_end(Some(3..5), &[(3, false), (2, true)])]
    fn text_runs_underline_the_composition(
        #[case] composition: Option<Range<usize>>,
        #[case] want: &[(usize, bool)],
    ) {
        let runs = text_runs(5, composition, &TextStyle::default().to_run(5));
        let got: Vec<_> = runs
            .iter()
            .map(|run| (run.len, run.underline.is_some()))
            .collect();
        assert_eq!(got, want);
    }
}
