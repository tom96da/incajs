// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Redraws a window when a state change reaches a style rule.

use gpui::Window;

use super::EventDispatcher;

impl EventDispatcher {
    /// Redraws the window when `changed` and a connected rule names the pointer state (`pointer`)
    /// or the focus state. Returns whether it requested the redraw.
    pub(crate) fn restyle(&self, changed: bool, pointer: bool, window: &mut Window) -> bool {
        let tree = &self.host.borrow().tree;
        let redraw = changed
            && if pointer {
                tree.uses_pointer_state()
            } else {
                tree.uses_focus_state()
            };
        if redraw {
            window.refresh();
        }
        redraw
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use gpui::TestAppContext;
    use inca_test_macros::gpui_cases;

    use crate::tests::{add_sheet, dispatcher_with_engine};

    const FOCUS_RULE: &str = "input:focus { width: 60px }";
    const HOVER_RULE: &str = "div:hover { width: 60px }";

    #[gpui_cases]
    #[gpui::test]
    #[case::focus_changed_with_a_focus_rule(true, false, FOCUS_RULE, true)]
    #[case::focus_changed_without_a_rule(true, false, "", false)]
    #[case::focus_unchanged_with_a_focus_rule(false, false, FOCUS_RULE, false)]
    #[case::focus_changed_with_only_a_hover_rule(true, false, HOVER_RULE, false)]
    #[case::pointer_changed_with_a_hover_rule(true, true, HOVER_RULE, true)]
    #[case::pointer_changed_with_only_a_focus_rule(true, true, FOCUS_RULE, false)]
    fn restyle_redraws_only_for_a_changed_value_a_rule_reads(
        cx: &mut TestAppContext,
        #[case] changed: bool,
        #[case] pointer: bool,
        #[case] css: &str,
        #[case] expected: bool,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        add_sheet(&host, css);
        let cx = cx.add_empty_window();

        let redrawn = cx.update(|window, _| dispatcher.restyle(changed, pointer, window));

        assert_eq!(redrawn, expected);
    }
}
