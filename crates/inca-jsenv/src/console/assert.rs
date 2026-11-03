// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `console.assert`.

use std::rc::Rc;

use rquickjs::function::{Opt, Rest};
use rquickjs::{Coerced, Ctx, Value};

use super::{Console, Level, format};

/// The `console.assert` line: "Assertion failed", joined to a string first
/// datum with `: `.
fn assertion_message<'js>(ctx: &Ctx<'js>, mut data: Vec<Value<'js>>, color: bool) -> String {
    const MESSAGE: &str = "Assertion failed";
    let first = data
        .first()
        .and_then(Value::as_string)
        .and_then(|text| text.to_string().ok());
    let lead = match first {
        Some(first) => {
            data.remove(0);
            format!("{MESSAGE}: {first}")
        }
        None => MESSAGE.to_owned(),
    };
    match rquickjs::String::from_str(ctx.clone(), &lead) {
        Ok(lead) => data.insert(0, lead.into_value()),
        Err(_) => return MESSAGE.to_owned(),
    }
    format::substitute(ctx, &data, color)
}

/// `console.assert`: an error line when the condition is falsy.
pub(super) fn assert<'js>(c: Rc<Console>) -> impl Fn(Ctx<'js>, Opt<Value<'js>>, Rest<Value<'js>>) {
    move |ctx, condition, data| {
        let holds = condition
            .0
            .and_then(|value| value.get::<Coerced<bool>>().ok())
            .is_some_and(|flag| flag.0);
        if holds {
            return;
        }
        c.write(Level::Error, &assertion_message(&ctx, data.0, c.color));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tests::texts;

    #[test]
    fn assert_is_silent_when_the_condition_holds() {
        assert!(
            texts("console.assert(true, 'no'); console.assert(1); console.assert('x');").is_empty()
        );
    }

    #[test]
    fn assert_treats_a_missing_or_falsy_condition_as_failed() {
        assert_eq!(
            texts("console.assert(); console.assert(0); console.assert(''); console.assert(null);"),
            ["✖ Assertion failed"; 4]
        );
    }

    #[test]
    fn assert_joins_a_string_first_datum_with_a_colon() {
        assert_eq!(
            texts("console.assert(false, 'bad', 2);"),
            ["✖ Assertion failed: bad 2"]
        );
    }

    #[test]
    fn assert_puts_non_string_data_after_the_message() {
        assert_eq!(
            texts("console.assert(false, { a: 1 }, 'x');"),
            ["✖ Assertion failed { a: 1 } x"]
        );
    }

    #[test]
    fn assert_formats_its_message_and_data() {
        assert_eq!(
            texts(
                r"
                console.assert(false, '%s is %d', 'x', 4, 'end');
                console.assert(false, 'plain %s');
                console.assert(false, '%o', 'q');
                console.assert(false, '%s', 'a');
                console.assert(false, 7, '%s', 'b');
                console.assert(false, { a: 1 });
                console.assert(false, 'v', '%s');
                "
            ),
            [
                "✖ Assertion failed: x is 4 end",
                "✖ Assertion failed: plain %s",
                "✖ Assertion failed: 'q'",
                "✖ Assertion failed: a",
                "✖ Assertion failed 7 %s b",
                "✖ Assertion failed { a: 1 }",
                "✖ Assertion failed: v %s",
            ]
        );
    }

    #[test]
    fn assert_does_not_format_when_the_condition_holds() {
        assert!(
            texts("console.assert(true, '%s', { toString() { console.log('ran'); } });").is_empty()
        );
    }
}
