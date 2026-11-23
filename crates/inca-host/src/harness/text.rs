// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The input method and clipboard operations of the [`super::Harness`].

use std::rc::Rc;

use gpui::{ClipboardItem, InputHandler};
use inca_gpui::TextInputHandler;
use inca_node::NodeId;
use inca_style::{ElementTag, build_spec};
use inca_window::EventDispatcher;

use super::Harness;

impl Harness {
    /// Sets the composition of the focused `input` to `text`. It replaces the current
    /// composition, or the selection when there is none, and leaves the caret
    /// after `text`. No key event is sent.
    ///
    /// # Panics
    ///
    /// Panics when the focused node is not an `input` that accepts text.
    pub fn ime_mark(&mut self, text: &str) {
        let end = text.encode_utf16().count();
        self.with_handler(|handler, window, cx| {
            handler.replace_and_mark_text_in_range(None, text, Some(end..end), window, cx);
        });
    }

    /// Inserts `text` in place of the composition of the focused `input`,
    /// which ends it. With no composition, `text` replaces the selection.
    ///
    /// # Panics
    ///
    /// Panics when the focused node is not an `input` that accepts text.
    pub fn ime_commit(&mut self, text: &str) {
        self.with_handler(|handler, window, cx| {
            handler.replace_text_in_range(None, text, window, cx);
        });
    }

    /// Ends the composition of the focused `input` and keeps its text.
    ///
    /// # Panics
    ///
    /// Panics when the focused node is not an `input` that accepts text.
    pub fn ime_unmark(&mut self) {
        self.with_handler(InputHandler::unmark_text);
    }

    /// Replaces the platform clipboard with the plain text `text`.
    pub fn set_clipboard(&mut self, text: &str) {
        self.cx
            .write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
    }

    /// The text on the platform clipboard, `None` when it is empty or holds no text.
    pub fn clipboard(&mut self) -> Option<String> {
        self.cx.read_from_clipboard().and_then(|item| item.text())
    }

    /// Sends `text` to the focused `input`, editable or not, as typed text. Sends
    /// nothing when the focused node is no `input`.
    pub(super) fn type_text(&mut self, text: &str) {
        let Some((node, editable)) = self.focused_input() else {
            return;
        };
        let mut handler = self.handler_for(node, editable);
        self.cx
            .update(|window, cx| handler.replace_text_in_range(None, text, window, cx));
        self.cx.run_until_parked();
    }

    /// An input handler for `node`, wired to the dispatcher of the open window.
    fn handler_for(&mut self, node: NodeId, editable: bool) -> TextInputHandler<EventDispatcher> {
        let dispatcher = self
            .window
            .update(&mut self.cx, |app, _, _| app.session.dispatcher.clone())
            .expect("the window is open");
        TextInputHandler::new(dispatcher, node, editable)
    }

    /// The focused node and whether it accepts text, when it is an `input`.
    pub(super) fn focused_input(&mut self) -> Option<(NodeId, bool)> {
        let cells = Rc::clone(&self.cells);
        self.cx.update(|window, cx| {
            let document = cells.document.borrow();
            let node = cells.focused_node(window, cx)?;
            match build_spec(document.tree(), document.style(), node)?.tag {
                ElementTag::Input { editable, .. } => Some((node, editable)),
                _ => None,
            }
        })
    }

    /// Runs `call` on the input handler of the focused `input`, then settles.
    fn with_handler(
        &mut self,
        call: impl FnOnce(&mut TextInputHandler<EventDispatcher>, &mut gpui::Window, &mut gpui::App),
    ) {
        let Some((node, true)) = self.focused_input() else {
            panic!("no focused editable input");
        };
        let mut handler = self.handler_for(node, true);
        self.cx.update(|window, cx| call(&mut handler, window, cx));
        self.cx.run_until_parked();
        self.settle();
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;
    use inca_test_macros::gpui_cases;

    use super::*;
    use crate::tests::{KEYED, eval_string, load_listening, log};

    /// A focused 100x20 `input` held in `globalThis.field`. Its `keydown`,
    /// `compositionupdate` and `compositionend` listeners log `type:data`.
    const FIELD: &str = r"
            const n = __inca_native__;
            const field = n.createNode('input');
            n.setStyle(field, 'width', 100);
            n.setStyle(field, 'height', 20);
            n.appendChild(n.rootNodeId(), field);
            globalThis.field = field;
            globalThis.log = [];
            globalThis.cb = {
                0: (e) => {
                    globalThis.log.push(`${e.type}:${e.data}`);
                },
            };
            listen(field, 'keydown', 0);
            listen(field, 'compositionupdate', 0);
            listen(field, 'compositionend', 0);
            n.focusNode(field);
        ";

    /// Cancels the `keydown` of the `input` of [`FIELD`].
    const CANCELED: &str = r"
            globalThis.cb[1] = (e) => e.preventDefault();
            listen(field, 'keydown', 1);
        ";

    /// Makes the `input` of [`FIELD`] `readonly`.
    const LOCKED: &str = "n.setAttribute(field, 'readonly', '');";

    /// Gives the `input` of [`FIELD`] the value `abc`.
    const ABC: &str = "n.setInputValue(field, 'abc');";

    fn value(h: &Harness) -> String {
        eval_string(h, "__inca_native__.inputValue(globalThis.field)")
    }

    #[gpui::test]
    fn ime_mark_sets_the_composition(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, FIELD);

        h.ime_mark("か");

        assert_eq!(log(&h), "compositionupdate:か");
    }

    #[gpui::test]
    fn ime_commit_replaces_the_composition(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, FIELD);

        h.ime_mark("か");
        h.ime_commit("化");

        assert_eq!(
            log(&h),
            "compositionupdate:か,compositionupdate:化,compositionend:化"
        );
    }

    #[gpui::test]
    fn ime_unmark_keeps_the_composed_text(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, FIELD);

        h.ime_mark("か");
        h.ime_unmark();

        assert_eq!(log(&h), "compositionupdate:か,compositionend:か");
    }

    #[gpui::test]
    fn key_repeat_types_into_the_focused_input(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, FIELD);

        h.key_repeat("a");

        assert_eq!(value(&h), "a");
    }

    #[gpui::test]
    fn key_repeat_sends_no_text_after_a_canceled_keydown(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, &format!("{FIELD}{CANCELED}"));

        h.key_repeat("a");

        assert_eq!(value(&h), "");
    }

    #[gpui::test]
    fn key_repeat_sends_no_text_for_a_key_without_text(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, FIELD);

        h.key_repeat("escape");

        assert_eq!(value(&h), "");
    }

    #[gpui::test]
    fn key_repeat_types_nothing_into_a_readonly_input(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, &format!("{FIELD}{LOCKED}"));

        h.key_repeat("a");

        assert_eq!(value(&h), "");
    }

    #[gpui_cases]
    #[gpui::test]
    #[should_panic(expected = "no focused editable input")]
    #[case::mark_on_a_div(KEYED.to_owned(), |h| h.ime_mark("か"))]
    #[case::mark_on_a_readonly_input(format!("{FIELD}{LOCKED}"), |h| h.ime_mark("か"))]
    #[case::commit_on_a_readonly_input(format!("{FIELD}{LOCKED}"), |h| h.ime_commit("か"))]
    #[case::unmark_on_a_readonly_input(format!("{FIELD}{LOCKED}"), |h| h.ime_unmark())]
    fn ime_operations_panic_without_a_focused_editable_input(
        cx: &mut TestAppContext,
        #[case] fixture: String,
        #[case] operation: fn(&mut Harness),
    ) {
        let mut h = load_listening(cx, &fixture);

        operation(&mut h);
    }

    #[gpui::test]
    fn ime_mark_again_replaces_the_composition(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, FIELD);

        h.ime_mark("か");
        h.ime_mark("かな");

        assert_eq!(value(&h), "かな");
    }

    #[gpui::test]
    fn ime_mark_leaves_the_caret_after_the_text(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, FIELD);

        h.ime_mark("かな");
        h.ime_unmark();
        h.keystrokes("b");

        assert_eq!(value(&h), "かなb");
    }

    #[gpui::test]
    fn ime_mark_replaces_the_selection(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, &format!("{FIELD}{ABC}"));

        h.keystrokes("secondary-a");
        h.ime_mark("か");

        assert_eq!(value(&h), "か");
    }

    #[gpui::test]
    fn ime_commit_without_a_composition_replaces_the_selection(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, &format!("{FIELD}{ABC}"));

        h.keystrokes("secondary-a");
        h.ime_commit("x");

        assert_eq!(value(&h), "x");
    }

    #[gpui::test]
    fn clipboard_reads_what_the_app_copied(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, &format!("{FIELD}{ABC}"));

        h.keystrokes("secondary-a secondary-c");

        assert_eq!(h.clipboard().as_deref(), Some("abc"));
    }

    #[gpui::test]
    fn set_clipboard_is_what_the_app_pastes(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, FIELD);

        h.set_clipboard("pasted");
        h.keystrokes("secondary-v");

        assert_eq!(value(&h), "pasted");
    }

    #[gpui::test]
    fn clipboard_is_none_before_any_write(cx: &mut TestAppContext) {
        let mut h = load_listening(cx, FIELD);

        assert_eq!(h.clipboard(), None);
    }
}
