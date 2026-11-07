// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Redraws a window when a state change reaches a style rule.

use gpui::Window;

use super::EventDispatcher;

impl EventDispatcher {
    /// Redraws the window when `changed` and a connected rule names the focus state. Returns
    /// whether it requested the redraw.
    pub(crate) fn restyle(&self, changed: bool, window: &mut Window) -> bool {
        let redraw = changed && self.host.borrow().tree.uses_focus_state();
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

    #[gpui_cases]
    #[gpui::test]
    #[case::changed_with_a_focus_rule(true, FOCUS_RULE, true)]
    #[case::changed_without_a_rule(true, "", false)]
    #[case::unchanged_with_a_focus_rule(false, FOCUS_RULE, false)]
    #[case::changed_with_only_a_hover_rule(true, "div:hover { width: 60px }", false)]
    fn restyle_redraws_only_for_a_changed_value_a_rule_reads(
        cx: &mut TestAppContext,
        #[case] changed: bool,
        #[case] css: &str,
        #[case] expected: bool,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        add_sheet(&host, css);
        let cx = cx.add_empty_window();

        let redrawn = cx.update(|window, _| dispatcher.restyle(changed, window));

        assert_eq!(redrawn, expected);
    }
}
