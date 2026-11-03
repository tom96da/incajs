// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `console` counters and timers.

use std::rc::Rc;

use rquickjs::function::{Opt, Rest};
use rquickjs::{Coerced, Value};

use super::{Console, Level};
use crate::inspect;

/// The label `count`, `time` and their relatives use when given none.
const DEFAULT_LABEL: &str = "default";

/// A `label` argument: `"default"` when absent, `undefined` or unstringifiable.
fn label_of(label: &Opt<Value<'_>>) -> String {
    match &label.0 {
        Some(value) if !value.is_undefined() => {
            inspect::settled(value.ctx(), value.get::<Coerced<String>>())
                .map_or_else(|| DEFAULT_LABEL.to_owned(), |text| text.0)
        }
        _ => DEFAULT_LABEL.to_owned(),
    }
}

/// `console.count`: increments the label's counter and prints it.
pub(super) fn count(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>) {
    move |label| {
        let label = label_of(&label);
        let count = c.state.borrow_mut().count(&label);
        c.write(Level::Log, &format!("{label}: {count}"));
    }
}

/// `console.countReset`: zeroes the label's counter, warning when it has none.
pub(super) fn count_reset(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>) {
    move |label| {
        let label = label_of(&label);
        if !c.state.borrow_mut().reset_count(&label) {
            c.warn(&format!("Count for '{label}' does not exist"));
        }
    }
}

/// `console.time`: starts the label's timer, warning when it is running.
pub(super) fn time(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>) {
    move |label| {
        let label = label_of(&label);
        if !c.state.borrow_mut().start_timer(&label) {
            c.warn(&format!("Timer '{label}' already exists"));
        }
    }
}

/// `console.timeLog`: prints the label's elapsed time and the extra data.
pub(super) fn time_log(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>, Rest<Value<'_>>) {
    move |label, data| c.time_log(&label_of(&label), &data.0, false)
}

/// `console.timeEnd`: prints the label's elapsed time and stops the timer.
pub(super) fn time_end(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>) {
    move |label| c.time_log(&label_of(&label), &[], true)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tests::{texts, without_elapsed};

    #[test]
    fn a_label_that_cannot_be_stringified_falls_back_to_default() {
        assert_eq!(texts("console.count(Symbol('s'));"), ["default: 1"]);
    }

    #[test]
    fn a_label_whose_to_string_throws_falls_back_to_default() {
        assert_eq!(
            texts("console.count({ toString() { throw new Error('no'); } });"),
            ["default: 1"]
        );
    }

    #[test]
    fn a_non_string_label_is_stringified() {
        assert_eq!(
            texts("console.count(7); console.count(null);"),
            ["7: 1", "null: 1"]
        );
    }

    #[test]
    fn time_log_prints_its_extra_data_without_substituting_specifiers() {
        let lines = texts(
            r"
            console.time('%s'); console.timeLog('%s', '%d items', '3');
            console.timeLog('%s', 'a', '%s');
            ",
        );

        assert_eq!(without_elapsed(&lines[0]), "%s: Nms %d items 3");
        assert_eq!(without_elapsed(&lines[1]), "%s: Nms a %s");
    }
}
