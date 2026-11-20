// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The input method operations of the [`super::Harness`].

use std::rc::Rc;

use gpui::InputHandler;
use inca_bridge::EventDispatcher;
use inca_gpui::TextInputHandler;
use inca_node::{ElementTag, NodeId, build_spec};

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

    /// The focused node and whether it accepts text, when it is an `input`.
    pub(super) fn focused_input(&mut self) -> Option<(NodeId, bool)> {
        let host = Rc::clone(&self.host);
        self.cx.update(|window, cx| {
            let host = host.borrow();
            let node = host.focus.focused_node(window, cx)?;
            match build_spec(&host.tree, node)?.tag {
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
        let dispatcher = self
            .window
            .update(&mut self.cx, |app, _, _| app.session.dispatcher.clone())
            .expect("the window is open");
        let mut handler = TextInputHandler::new(dispatcher, node, true);
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
    use crate::harness::tests::KEYED;
    use crate::tests::{eval_string, load_listening, log};

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
}
