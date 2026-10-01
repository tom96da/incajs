// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `globalThis.console`.

mod format;
mod state;
mod table;

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{
    Coerced, Ctx, Function, Object, Result as JsResult, Value,
    function::{Opt, Rest},
};
use unicode_width::UnicodeWidthStr;

use crate::inspect;
use state::State;

/// Starts every line, once per open group.
const GROUP_INDENT: &str = "│ ";

/// The label `count`, `time` and their relatives use when given none.
const DEFAULT_LABEL: &str = "default";

/// The kind of line `console` produced. Nothing here filters on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Trace,
    Debug,
    Log,
    Info,
    Warn,
    Error,
}

impl Level {
    /// The marker a line of this level starts with, after the group guides.
    fn symbol(self) -> &'static str {
        match self {
            Level::Error => "✖ ",
            Level::Warn => "⚠ ",
            Level::Info => "ℹ ",
            Level::Trace | Level::Debug | Level::Log => "",
        }
    }
}

/// Where a formatted `console` line goes.
pub type Output = Rc<dyn Fn(Level, &str)>;

/// An [`Output`] writing each line to stderr, bypassing `log`'s level filter.
#[must_use]
pub fn to_stderr() -> Output {
    Rc::new(|_, message| eprintln!("{message}"))
}

struct Console {
    output: Output,
    state: RefCell<State>,
}

impl Console {
    /// Writes `body` behind the group guides and the level's symbol; later
    /// lines align under the first one's text.
    fn write(&self, level: Level, body: &str) {
        let guides = GROUP_INDENT.repeat(self.state.borrow().depth());
        let symbol = level.symbol();
        let pad = " ".repeat(symbol.width());
        let text = body
            .split('\n')
            .enumerate()
            .map(|(index, line)| {
                let lead = if index == 0 { symbol } else { pad.as_str() };
                let text = format!("{guides}{lead}{line}");
                if line.is_empty() {
                    text.trim_end().to_owned()
                } else {
                    text
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        (self.output)(level, &text);
    }

    fn warn(&self, message: &str) {
        self.write(Level::Warn, message);
    }

    fn group(&self, ctx: &Ctx<'_>, marker: &str, label: &[Value<'_>]) {
        let label = if label.is_empty() {
            "console.group".to_owned()
        } else {
            format::line(ctx, label)
        };
        let header = format!("{marker} {label}");
        self.write(Level::Log, header.trim_end());
        self.state.borrow_mut().open_group();
    }

    fn time_log(&self, label: &str, data: &[Value<'_>], end: bool) {
        let elapsed = if end {
            self.state.borrow_mut().end_timer(label)
        } else {
            self.state.borrow().elapsed_ms(label)
        };
        let Some(elapsed) = elapsed else {
            self.warn(&format!("Timer '{label}' does not exist"));
            return;
        };
        let mut line = format!("{label}: {elapsed:.3}ms");
        if !data.is_empty() {
            line.push(' ');
            line.push_str(&inspect::line(data));
        }
        self.write(Level::Log, &line);
    }
}

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

/// The `console.assert` line: "Assertion failed", joined to a string first
/// datum with `: `.
fn assertion_message<'js>(ctx: &Ctx<'js>, mut data: Vec<Value<'js>>) -> String {
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
    format::line(ctx, &data)
}

/// The frames of the stack that called `console.trace`. `QuickJS` lists no
/// frame for a native function, so skipping this evaluation's own leaves the
/// caller on top.
fn caller_stack(ctx: &Ctx<'_>) -> Vec<String> {
    let stack =
        inspect::settled(ctx, ctx.eval::<String, _>("new Error().stack")).unwrap_or_default();
    stack
        .lines()
        .map(str::trim_end)
        .skip(1)
        .map(str::to_owned)
        .collect()
}

/// Installs `globalThis.console`, writing through `output`.
///
/// Install before evaluating any script, so top-level logging works.
///
/// # Errors
///
/// Returns an error if defining `console` or any of its methods fails.
pub fn install(ctx: &Ctx<'_>, output: &Output) -> JsResult<()> {
    let console = Object::new(ctx.clone())?;
    let shared = Rc::new(Console {
        output: Rc::clone(output),
        state: RefCell::new(State::default()),
    });

    install_lines(ctx, &console, &shared)?;
    install_counting(ctx, &console, &shared)?;
    install_timing(ctx, &console, &shared)?;
    install_structure(ctx, &console, &shared)?;

    ctx.globals().set("console", console)?;
    Ok(())
}

fn install_lines<'js>(ctx: &Ctx<'js>, console: &Object<'js>, shared: &Rc<Console>) -> JsResult<()> {
    for (name, level) in [
        ("debug", Level::Debug),
        ("log", Level::Log),
        ("dirxml", Level::Log),
        ("info", Level::Info),
        ("warn", Level::Warn),
        ("error", Level::Error),
    ] {
        let c = Rc::clone(shared);
        console.set(
            name,
            Function::new(ctx.clone(), move |ctx: Ctx<'_>, values: Rest<Value<'_>>| {
                c.write(level, &format::line(&ctx, &values.0));
            })?,
        )?;
    }

    let c = Rc::clone(shared);
    console.set(
        "trace",
        Function::new(ctx.clone(), move |ctx: Ctx<'_>, values: Rest<Value<'_>>| {
            let mut text = "Trace".to_owned();
            if !values.0.is_empty() {
                text.push_str(": ");
                text.push_str(&format::line(&ctx, &values.0));
            }
            for frame in caller_stack(&ctx) {
                text.push('\n');
                text.push_str(&frame);
            }
            c.write(Level::Trace, &text);
        })?,
    )?;

    Ok(())
}

fn install_counting<'js>(
    ctx: &Ctx<'js>,
    console: &Object<'js>,
    shared: &Rc<Console>,
) -> JsResult<()> {
    let c = Rc::clone(shared);
    console.set(
        "assert",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>, condition: Opt<Value<'js>>, data: Rest<Value<'js>>| {
                let holds = condition
                    .0
                    .and_then(|value| value.get::<Coerced<bool>>().ok())
                    .is_some_and(|flag| flag.0);
                if holds {
                    return;
                }
                c.write(Level::Error, &assertion_message(&ctx, data.0));
            },
        )?,
    )?;

    let c = Rc::clone(shared);
    console.set(
        "count",
        Function::new(ctx.clone(), move |label: Opt<Value<'_>>| {
            let label = label_of(&label);
            let count = c.state.borrow_mut().count(&label);
            c.write(Level::Log, &format!("{label}: {count}"));
        })?,
    )?;

    let c = Rc::clone(shared);
    console.set(
        "countReset",
        Function::new(ctx.clone(), move |label: Opt<Value<'_>>| {
            let label = label_of(&label);
            if !c.state.borrow_mut().reset_count(&label) {
                c.warn(&format!("Count for '{label}' does not exist"));
            }
        })?,
    )?;

    Ok(())
}

fn install_timing<'js>(
    ctx: &Ctx<'js>,
    console: &Object<'js>,
    shared: &Rc<Console>,
) -> JsResult<()> {
    let c = Rc::clone(shared);
    console.set(
        "time",
        Function::new(ctx.clone(), move |label: Opt<Value<'_>>| {
            let label = label_of(&label);
            if !c.state.borrow_mut().start_timer(&label) {
                c.warn(&format!("Timer '{label}' already exists"));
            }
        })?,
    )?;

    let c = Rc::clone(shared);
    console.set(
        "timeLog",
        Function::new(
            ctx.clone(),
            move |label: Opt<Value<'_>>, data: Rest<Value<'_>>| {
                c.time_log(&label_of(&label), &data.0, false);
            },
        )?,
    )?;

    let c = Rc::clone(shared);
    console.set(
        "timeEnd",
        Function::new(ctx.clone(), move |label: Opt<Value<'_>>| {
            c.time_log(&label_of(&label), &[], true);
        })?,
    )?;

    Ok(())
}

fn install_structure<'js>(
    ctx: &Ctx<'js>,
    console: &Object<'js>,
    shared: &Rc<Console>,
) -> JsResult<()> {
    let c = Rc::clone(shared);
    console.set(
        "group",
        Function::new(ctx.clone(), move |ctx: Ctx<'_>, label: Rest<Value<'_>>| {
            c.group(&ctx, "▼", &label.0);
        })?,
    )?;

    let c = Rc::clone(shared);
    console.set(
        "groupCollapsed",
        Function::new(ctx.clone(), move |ctx: Ctx<'_>, label: Rest<Value<'_>>| {
            c.group(&ctx, "▶", &label.0);
        })?,
    )?;

    let c = Rc::clone(shared);
    console.set(
        "groupEnd",
        Function::new(ctx.clone(), move || c.state.borrow_mut().close_group())?,
    )?;

    let c = Rc::clone(shared);
    console.set(
        "clear",
        Function::new(ctx.clone(), move || c.state.borrow_mut().close_all_groups())?,
    )?;

    let c = Rc::clone(shared);
    console.set(
        "dir",
        Function::new(ctx.clone(), move |item: Opt<Value<'_>>| {
            let text = item
                .0
                .as_ref()
                .map_or_else(|| "undefined".to_owned(), inspect::quoted);
            c.write(Level::Log, &text);
        })?,
    )?;

    let c = Rc::clone(shared);
    console.set(
        "table",
        Function::new(
            ctx.clone(),
            move |data: Opt<Value<'_>>, columns: Opt<Value<'_>>| {
                let drawn = data
                    .0
                    .as_ref()
                    .and_then(|data| table::render(data, columns.0.as_ref()));
                let text = drawn.unwrap_or_else(|| {
                    data.0.as_ref().map_or_else(
                        || "undefined".to_owned(),
                        |v| inspect::line(std::slice::from_ref(v)),
                    )
                });
                c.write(Level::Log, &text);
            },
        )?,
    )?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::cell::RefCell;

    use rquickjs::{Context, Runtime};

    use super::*;

    /// Evaluates `source` against a realm with `console` installed, and
    /// returns every line it wrote.
    fn lines_written_by(source: &str) -> Vec<(Level, String)> {
        let written = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&written);
        let output: Output = Rc::new(move |level, message| {
            sink.borrow_mut().push((level, message.to_owned()));
        });

        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            install(&ctx, &output).unwrap();
            ctx.eval::<(), _>(source).unwrap();
            assert!(!ctx.has_exception(), "a call left an exception pending");
        });

        written.take()
    }

    #[test]
    fn every_method_writes_at_its_own_level() {
        let written = lines_written_by(
            "console.trace('a'); console.debug('a'); console.log('a'); \
             console.info('a'); console.warn('a'); console.error('a');",
        );

        let levels: Vec<Level> = written.iter().map(|(level, _)| *level).collect();
        assert_eq!(
            levels,
            vec![
                Level::Trace,
                Level::Debug,
                Level::Log,
                Level::Info,
                Level::Warn,
                Level::Error
            ]
        );
    }

    #[test]
    fn arguments_are_joined_by_a_space() {
        let written = lines_written_by("console.log('a', 1, true);");

        assert_eq!(written[0].1, "a 1 true");
    }

    #[test]
    fn logging_nothing_writes_an_empty_line() {
        let written = lines_written_by("console.log();");

        assert_eq!(written.len(), 1);
        assert_eq!(written[0].1, "");
    }

    /// The `(level, text)` pairs `source` wrote, joined into one string per
    /// line for compact comparison.
    fn texts(source: &str) -> Vec<String> {
        lines_written_by(source)
            .into_iter()
            .map(|(_, t)| t)
            .collect()
    }

    /// Replaces the number in `label: 1.234ms` so a line can be compared.
    fn without_elapsed(line: &str) -> String {
        let (label, rest) = line.split_once(": ").unwrap();
        let (number, tail) = rest.split_once("ms").unwrap();
        assert!(number.parse::<f64>().is_ok(), "{line}");
        assert_eq!(number.split_once('.').unwrap().1.len(), 3, "{line}");
        format!("{label}: Nms{tail}")
    }

    #[test]
    fn symbols_mark_error_warn_and_info_only() {
        assert_eq!(
            texts(
                "console.error('e'); console.warn('w'); console.info('i'); \
                 console.log('l'); console.debug('d'); console.dirxml('x');"
            ),
            ["✖ e", "⚠ w", "ℹ i", "l", "d", "x"]
        );
    }

    #[test]
    fn a_multi_line_body_lines_up_under_the_first_lines_text() {
        assert_eq!(
            texts("console.error('a\\nb'); console.group('g'); console.warn('c\\nd');"),
            ["✖ a\n  b", "▼ g", "│ ⚠ c\n│   d"]
        );
    }

    #[test]
    fn dirxml_writes_at_log_level() {
        assert_eq!(lines_written_by("console.dirxml(1);")[0].0, Level::Log);
    }

    #[test]
    fn assert_is_silent_when_the_condition_holds() {
        assert!(
            texts("console.assert(true, 'no'); console.assert(1); console.assert('x');").is_empty()
        );
    }

    #[test]
    fn assert_prints_at_error_level_when_it_fails() {
        let written = lines_written_by("console.assert(false);");

        assert_eq!(written, [(Level::Error, "✖ Assertion failed".to_owned())]);
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
    fn count_runs_per_label_and_defaults_to_default() {
        assert_eq!(
            texts(
                "console.count(); console.count('a'); console.count(); console.count('a'); console.count(undefined);"
            ),
            ["default: 1", "a: 1", "default: 2", "a: 2", "default: 3"]
        );
    }

    #[test]
    fn count_reset_restarts_the_count() {
        assert_eq!(
            texts(
                "console.count('a'); console.count('a'); console.countReset('a'); console.count('a');"
            ),
            ["a: 1", "a: 2", "a: 1"]
        );
        assert_eq!(
            texts("console.count(); console.countReset(); console.count();"),
            ["default: 1", "default: 1"]
        );
    }

    #[test]
    fn count_reset_warns_about_an_unknown_label() {
        let written = lines_written_by("console.countReset('nope'); console.countReset();");

        assert_eq!(
            written,
            [
                (Level::Warn, "⚠ Count for 'nope' does not exist".to_owned()),
                (
                    Level::Warn,
                    "⚠ Count for 'default' does not exist".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn time_log_prints_the_label_and_a_millisecond_figure() {
        let lines = texts("console.time('t'); console.timeLog('t'); console.timeLog('t');");

        assert_eq!(lines.len(), 2);
        assert_eq!(without_elapsed(&lines[0]), "t: Nms");
        assert_eq!(without_elapsed(&lines[1]), "t: Nms");
    }

    #[test]
    fn time_log_appends_extra_data() {
        let lines = texts("console.time(); console.timeLog('default', 'x', { a: 1 });");

        assert_eq!(without_elapsed(&lines[0]), "default: Nms x { a: 1 }");
    }

    #[test]
    fn time_end_prints_then_forgets_the_timer() {
        let written = lines_written_by(
            "console.time('t'); console.timeEnd('t'); console.timeEnd('t'); console.timeLog('t');",
        );

        assert_eq!(without_elapsed(&written[0].1), "t: Nms");
        assert_eq!(written[0].0, Level::Log);
        assert_eq!(
            written[1..],
            [
                (Level::Warn, "⚠ Timer 't' does not exist".to_owned()),
                (Level::Warn, "⚠ Timer 't' does not exist".to_owned()),
            ]
        );
    }

    #[test]
    fn time_defaults_its_label() {
        let lines = texts("console.time(); console.timeEnd();");

        assert_eq!(without_elapsed(&lines[0]), "default: Nms");
    }

    #[test]
    fn a_duplicate_time_warns_and_keeps_the_first_timer() {
        let written =
            lines_written_by("console.time('t'); console.time('t'); console.timeEnd('t');");

        assert_eq!(
            written[0],
            (Level::Warn, "⚠ Timer 't' already exists".to_owned())
        );
        assert_eq!(written.len(), 2);
    }

    #[test]
    fn an_unknown_time_label_warns_for_both_log_and_end() {
        assert_eq!(
            texts("console.timeLog('x'); console.timeEnd();"),
            [
                "⚠ Timer 'x' does not exist",
                "⚠ Timer 'default' does not exist"
            ]
        );
    }

    #[test]
    fn groups_indent_what_follows_and_nest() {
        assert_eq!(
            texts(
                "console.group('a'); console.log('1'); console.group('b'); console.log('2'); \
                 console.groupEnd(); console.log('3'); console.groupEnd(); console.log('4');"
            ),
            ["▼ a", "│ 1", "│ ▼ b", "│ │ 2", "│ 3", "4"]
        );
    }

    #[test]
    fn a_collapsed_group_has_its_own_marker_and_still_indents() {
        assert_eq!(
            texts("console.groupCollapsed('c'); console.log('x');"),
            ["▶ c", "│ x"]
        );
    }

    #[test]
    fn a_group_label_is_formatted_like_log_arguments() {
        assert_eq!(texts("console.group('a', 1, true);"), ["▼ a 1 true"]);
    }

    #[test]
    fn a_group_without_a_label_is_named_after_itself() {
        assert_eq!(
            texts("console.group(); console.groupCollapsed();"),
            ["▼ console.group", "│ ▶ console.group"]
        );
    }

    #[test]
    fn group_end_with_no_open_group_does_nothing() {
        assert_eq!(
            texts(
                "console.groupEnd(); console.groupEnd(); console.log('x'); console.group('g'); console.groupEnd(); console.groupEnd(); console.log('y');"
            ),
            ["x", "▼ g", "y"]
        );
    }

    #[test]
    fn warnings_and_symbols_sit_behind_the_guides() {
        assert_eq!(
            texts("console.group('g'); console.countReset('q');"),
            ["▼ g", "│ ⚠ Count for 'q' does not exist"]
        );
    }

    #[test]
    fn clear_closes_every_group() {
        assert_eq!(
            texts("console.group('a'); console.group('b'); console.clear(); console.log('x');"),
            ["▼ a", "│ ▼ b", "x"]
        );
    }

    #[test]
    fn clear_keeps_counters_and_timers() {
        let lines = texts(
            "console.count(); console.time(); console.clear(); console.count(); console.timeEnd();",
        );

        assert_eq!(lines[0], "default: 1");
        assert_eq!(lines[1], "default: 2");
        assert_eq!(without_elapsed(&lines[2]), "default: Nms");
    }

    #[test]
    fn dir_quotes_a_string_and_ignores_options_and_extras() {
        assert_eq!(
            texts(
                "console.dir('s'); console.dir({ a: 'b' }, { depth: 0 }); console.dir(1, 2, 3); console.dir();"
            ),
            ["'s'", "{ a: 'b' }", "1", "undefined"]
        );
    }

    #[test]
    fn table_draws_through_the_console() {
        assert_eq!(
            texts("console.table([{ a: 1 }]);"),
            ["┌─────────┬───┐\n│ (index) │ a │\n├─────────┼───┤\n│ 0       │ 1 │\n└─────────┴───┘"]
        );
    }

    #[test]
    fn table_takes_a_columns_filter() {
        let drawn = &texts("console.table([{ a: 1, b: 2 }], ['b']);")[0];

        assert!(drawn.contains("│ b │"), "{drawn}");
        assert!(!drawn.contains("│ a │"), "{drawn}");
    }

    #[test]
    fn table_falls_back_to_log_for_a_non_object() {
        assert_eq!(
            texts("console.table(5); console.table('s'); console.table(null); console.table();"),
            ["5", "s", "null", "undefined"]
        );
    }

    #[test]
    fn table_writes_at_log_level_behind_the_guides() {
        let written = lines_written_by("console.group('g'); console.table([]);");

        assert_eq!(written[1].0, Level::Log);
        assert_eq!(
            written[1].1,
            "│ ┌─────────┐\n│ │ (index) │\n│ ├─────────┤\n│ └─────────┘"
        );
    }

    #[test]
    fn trace_prints_the_message_then_the_callers_frames() {
        let written = lines_written_by(
            "function inner() { console.trace('hi', 1); }\nfunction outer() { inner(); }\nouter();",
        );

        assert_eq!(written.len(), 1);
        assert_eq!(written[0].0, Level::Trace);
        let lines: Vec<&str> = written[0].1.lines().collect();
        assert_eq!(lines[0], "Trace: hi 1");
        assert!(lines[1].contains("at inner "), "{lines:?}");
        assert!(lines[2].contains("at outer "), "{lines:?}");
        assert!(
            !lines.iter().any(|l| l.contains("console.trace")),
            "{lines:?}"
        );
    }

    #[test]
    fn trace_without_data_is_just_the_word() {
        let text = &texts("console.trace();")[0];

        assert_eq!(text.lines().next(), Some("Trace"));
    }

    #[test]
    fn trace_stack_lines_follow_the_group_guides() {
        let text = &texts("console.group('g'); console.trace('x');")[1];

        assert!(text.lines().all(|l| l.starts_with("│ ")), "{text}");
    }

    #[test]
    fn empty_lines_carry_no_trailing_whitespace() {
        assert_eq!(
            texts(
                "console.group(''); console.log(''); console.log('a\\n\\nb'); \
                 console.error(''); console.error('a\\n\\nb'); console.log();"
            ),
            ["▼", "│", "│ a\n│\n│ b", "│ ✖", "│ ✖ a\n│\n│   b", "│"]
        );
        assert_eq!(texts("console.log('');"), [""]);
    }

    #[test]
    fn table_escapes_control_characters_and_backslashes_in_cells() {
        let drawn = &texts(r"console.table([{ 'k\n': 'a\nb\tc\\d\x01', e: '\u0085' }]);")[0];

        assert_eq!(drawn.lines().count(), 5, "{drawn}");
        assert!(drawn.contains(r"│ k\n "), "{drawn}");
        assert!(drawn.contains(r"'a\nb\tc\\d\x01'"), "{drawn}");
        assert!(drawn.contains(r"'\x85'"), "{drawn}");
    }

    #[test]
    fn table_treats_date_map_set_and_error_as_not_tabular() {
        assert_eq!(
            texts(
                "console.table(new Date(0)); console.table(new Map([[1, 2]])); \
                 console.table(new Set([1])); console.table(new Error('x')); console.table({});"
            )
            .iter()
            .map(|t| t.lines().next().unwrap().to_owned())
            .collect::<Vec<_>>(),
            ["{}", "{}", "{}", "Error: x", "{}"]
        );
    }

    #[test]
    fn table_puts_non_tabular_rows_in_the_values_column() {
        let drawn = &texts("console.table([new Date(0), new Error('x')]);")[0];

        assert!(drawn.contains("Values"), "{drawn}");
        assert!(drawn.contains("Error: x"), "{drawn}");
        assert!(drawn.contains(r"Error: x\n    at "), "{drawn}");
        assert_eq!(drawn.lines().count(), 6, "{drawn}");
    }

    #[test]
    fn table_reads_each_getter_once() {
        let lines = texts(
            "console.table([{ get a() { console.count('g'); return 1; } }, \
             { get a() { console.count('h'); return 2; } }], ['a', 'a']);",
        );

        assert_eq!(lines[..2], ["g: 1", "h: 1"]);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn table_ignores_a_getter_that_changes_the_group_depth() {
        let lines = texts("console.table([{ get a() { console.group('x'); return 1; } }]);");

        assert_eq!(lines[0], "▼ x");
        assert!(lines[1].starts_with("│ ┌"), "{}", lines[1]);
    }

    #[test]
    fn table_dedupes_repeated_columns() {
        assert_eq!(
            texts("console.table([{ a: 1 }], ['a', 'a']);"),
            texts("console.table([{ a: 1 }], ['a']);")
        );
    }

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
    fn every_formatting_method_substitutes_specifiers() {
        let written = lines_written_by(
            "console.log('%s=%d', 'a', 1); console.debug('%s', 'x', 'y'); \
             console.info('%o', 'i'); console.warn('%f', '1.5'); \
             console.error('%s', Symbol('e')); console.dirxml('%i', '7.9');",
        );

        assert_eq!(
            written,
            [
                (Level::Log, "a=1".to_owned()),
                (Level::Debug, "x y".to_owned()),
                (Level::Info, "ℹ 'i'".to_owned()),
                (Level::Warn, "⚠ 1.5".to_owned()),
                (Level::Error, "✖ Symbol(e)".to_owned()),
                (Level::Log, "7".to_owned()),
            ]
        );
    }

    #[test]
    fn a_lone_argument_is_never_formatted() {
        assert_eq!(
            texts("console.log('%s %%'); console.error('%d'); console.group('%c'); console.trace('%o');")
                .iter()
                .map(|t| t.lines().next().unwrap().to_owned())
                .collect::<Vec<_>>(),
            ["%s %%", "✖ %d", "▼ %c", "│ Trace: %o"]
        );
    }

    #[test]
    fn formatting_follows_the_group_guides_and_symbols() {
        assert_eq!(
            texts("console.group('g'); console.warn('%s\\n%s', 'a', 'b');"),
            ["▼ g", "│ ⚠ a\n│   b"]
        );
    }

    #[test]
    fn assert_formats_its_message_and_data() {
        assert_eq!(
            texts(
                "console.assert(false, '%s is %d', 'x', 4, 'end'); \
                 console.assert(false, 'plain %s'); \
                 console.assert(false, '%o', 'q'); \
                 console.assert(false, '%s', 'a'); \
                 console.assert(false, 7, '%s', 'b'); \
                 console.assert(false, { a: 1 }); \
                 console.assert(false, 'v', '%s');"
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

    #[test]
    fn a_group_label_substitutes_specifiers() {
        assert_eq!(
            texts(
                "console.group('%s (%d)', 'job', 3, 'extra'); console.groupCollapsed('%c%s', 'css', 'c'); console.group('%s');"
            ),
            ["▼ job (3) extra", "│ ▶ c", "│ │ ▼ %s"]
        );
    }

    #[test]
    fn trace_substitutes_specifiers_in_its_label() {
        let text = &texts("console.trace('%s:%d', 'f', 2, 'x');")[0];

        assert_eq!(text.lines().next(), Some("Trace: f:2 x"));
    }

    #[test]
    fn time_log_prints_its_extra_data_without_substituting_specifiers() {
        let lines = texts(
            "console.time('%s'); console.timeLog('%s', '%d items', '3'); \
             console.timeLog('%s', 'a', '%s');",
        );

        assert_eq!(without_elapsed(&lines[0]), "%s: Nms %d items 3");
        assert_eq!(without_elapsed(&lines[1]), "%s: Nms a %s");
    }

    #[test]
    fn dir_table_and_the_counters_do_not_format() {
        assert_eq!(
            texts(
                "console.dir('%s', 'x'); console.table('%s'); console.count('%s'); \
                 console.countReset('%d'); console.timeEnd('%i');"
            ),
            [
                "'%s'",
                "%s",
                "%s: 1",
                "⚠ Count for '%d' does not exist",
                "⚠ Timer '%i' does not exist",
            ]
        );
    }

    #[test]
    fn time_labels_are_not_formatted() {
        let lines = texts("console.time('%d'); console.timeEnd('%d');");

        assert_eq!(without_elapsed(&lines[0]), "%d: Nms");
    }

    #[test]
    fn a_throwing_conversion_leaves_no_exception_behind() {
        assert_eq!(
            texts(
                "console.log('%s', { toString() { throw new Error('no'); } }); \
                 try { null.x; } catch (e) { console.log('caught', e instanceof TypeError); }"
            ),
            ["{ toString: [Function: toString] }", "caught true"]
        );
    }

    #[test]
    fn unreadable_values_in_any_method_leave_nothing_pending() {
        assert_eq!(
            texts(
                "console.log({ get a() { throw 1; } }); \
                 console.log([Object.defineProperty([1], 0, { get() { throw 1; } })]); \
                 console.log(new Proxy({}, { ownKeys() { throw 1; } })); \
                 console.log(Object.defineProperty(new Error('e'), 'stack', { get() { throw 1; } })); \
                 console.dir({ get a() { throw 1; } }); \
                 try { null.x; } catch (e) { console.log('caught'); }"
            )
            .iter()
            .map(|t| t.lines().next().unwrap().to_owned())
            .collect::<Vec<_>>(),
            [
                "{ a: [unreadable] }",
                "[ [ [unreadable] ] ]",
                "{}",
                "Error: e",
                "{ a: [unreadable] }",
                "caught"
            ]
        );
    }

    #[test]
    fn replacing_the_global_parsers_does_not_change_formatting() {
        assert_eq!(
            texts(
                "globalThis.parseInt = () => 99; globalThis.parseFloat = () => 98; console.log('%d %i %f %s', '1', '2', '3.5', 4);"
            ),
            ["1 2 3.5 4"]
        );
    }

    #[test]
    fn console_is_reachable_before_anything_else_runs() {
        let written = lines_written_by("typeof console === 'object' && console.log('present');");

        assert_eq!(written[0].1, "present");
    }
}
