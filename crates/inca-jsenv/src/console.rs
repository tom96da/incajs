// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `globalThis.console` object: [`install`] defines it, and [`Output`]
//! receives what it prints.

mod format;
mod state;
mod table;

use std::cell::RefCell;
use std::ffi::OsStr;
use std::io::IsTerminal;
use std::rc::Rc;

use rquickjs::{
    Coerced, Ctx, Function, Object, Result as JsResult, Value,
    function::{Opt, Rest},
};
use unicode_width::UnicodeWidthStr;

use crate::inspect;
use crate::paint::{Style, paint};
use state::State;

/// Starts every line, once per open group.
const GROUP_INDENT: &str = "│ ";

/// The label `count`, `time` and their relatives use when given none.
const DEFAULT_LABEL: &str = "default";

/// The kind of line `console` produced, passed to [`Output`] with the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Written by `console.trace`.
    Trace,
    /// Written by `console.debug`.
    Debug,
    /// Written by `console.log`, `dirxml`, `dir`, `table`, `count`, the timer
    /// methods and `group`/`groupCollapsed`.
    Log,
    /// Written by `console.info`.
    Info,
    /// Written by `console.warn`, and by `countReset`, `time`, `timeLog` or
    /// `timeEnd` when the label is unknown, or for `time` already running.
    Warn,
    /// Written by `console.error` and by a failed `console.assert`.
    Error,
}

impl Level {
    /// The glyph and color a line of this level starts with.
    fn mark(self) -> Option<(&'static str, Style)> {
        match self {
            Level::Error => Some(("✖", Style::Red)),
            Level::Warn => Some(("⚠", Style::Yellow)),
            Level::Info => Some(("ℹ", Style::Blue)),
            Level::Trace | Level::Debug | Level::Log => None,
        }
    }
}

/// Receives each formatted `console` line with its [`Level`]. A line has no
/// trailing newline, and holds several newline-separated rows when the call
/// printed more than one.
pub type Output = Rc<dyn Fn(Level, &str)>;

/// An [`Output`] writing each line to stderr, bypassing `log`'s level filter.
#[must_use]
pub fn to_stderr() -> Output {
    Rc::new(|_, message| eprintln!("{message}"))
}

/// The color decision for the given variables and terminal state.
fn color_enabled(
    no_color: Option<&OsStr>,
    force_color: Option<&OsStr>,
    stderr_is_terminal: bool,
) -> bool {
    if no_color.is_some_and(|value| !value.is_empty()) {
        return false;
    }
    match force_color {
        Some(value) if value == "0" || value == "false" => false,
        Some(value) if !value.is_empty() => true,
        _ => stderr_is_terminal,
    }
}

/// Whether `console` output to stderr should be colored, as the environment
/// asks.
///
/// Color is off when `NO_COLOR` is non-empty, which wins over `FORCE_COLOR`.
/// Otherwise it is on when `FORCE_COLOR` is set to anything except `0`,
/// `false` or the empty string, and otherwise on when stderr is a terminal.
/// Pass the result to [`install`].
#[must_use]
pub fn color_from_env() -> bool {
    color_enabled(
        std::env::var_os("NO_COLOR").as_deref(),
        std::env::var_os("FORCE_COLOR").as_deref(),
        std::io::stderr().is_terminal(),
    )
}

struct Console {
    output: Output,
    state: RefCell<State>,
    color: bool,
}

impl Console {
    /// A debug line is dimmed whole, so its values stay uncolored.
    fn format(&self, ctx: &Ctx<'_>, level: Level, values: &[Value<'_>]) -> String {
        format::line(ctx, values, self.color && level != Level::Debug)
    }

    /// Writes `body` behind the group guides and the level's symbol; later
    /// lines align under the first one's text.
    fn write(&self, level: Level, body: &str) {
        let depth = self.state.borrow().depth();
        let guide = paint(self.color, Style::Grey, GROUP_INDENT);
        let guides = guide.repeat(depth);
        let bare_guides = match depth {
            0 => String::new(),
            _ => format!(
                "{}{}",
                guide.repeat(depth - 1),
                paint(self.color, Style::Grey, GROUP_INDENT.trim_end())
            ),
        };
        let pad = " ".repeat(level.mark().map_or(0, |(glyph, _)| glyph.width() + 1));
        let text = body
            .split('\n')
            .enumerate()
            .map(|(index, line)| {
                let mark = level.mark().filter(|_| index == 0);
                if line.is_empty() {
                    return match mark {
                        Some((glyph, style)) => {
                            format!("{guides}{}", paint(self.color, style, glyph))
                        }
                        None => bare_guides.clone(),
                    };
                }
                let lead = match mark {
                    Some((glyph, style)) => format!("{} ", paint(self.color, style, glyph)),
                    None => pad.clone(),
                };
                let line = if level == Level::Debug {
                    paint(self.color, Style::Grey, line)
                } else {
                    line.to_owned()
                };
                format!("{guides}{lead}{line}")
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
            self.format(ctx, Level::Log, label)
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
            line.push_str(&inspect::line(data, self.color));
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
    format::line(ctx, &data, color)
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

/// Defines `globalThis.console` with the WHATWG Console methods, writing each
/// formatted line through `output`.
///
/// With `color` set, the lines passed to `output` carry ANSI color sequences;
/// [`color_from_env`] gives the conventional value. Install before evaluating
/// any script, so top-level logging works.
///
/// # Errors
///
/// Returns an error if defining `console` or any of its methods fails, or if
/// the realm lacks an intrinsic `console` prints values with.
pub fn install(ctx: &Ctx<'_>, output: &Output, color: bool) -> JsResult<()> {
    inspect::pin(ctx)?;
    let console = Object::new(ctx.clone())?;
    let shared = Rc::new(Console {
        output: Rc::clone(output),
        state: RefCell::new(State::default()),
        color,
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
                c.write(level, &c.format(&ctx, level, &values.0));
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
                text.push_str(&c.format(&ctx, Level::Trace, &values.0));
            }
            for frame in caller_stack(&ctx) {
                text.push('\n');
                text.push_str(&paint(c.color, Style::Grey, &frame));
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
                c.write(Level::Error, &assertion_message(&ctx, data.0, c.color));
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
            let text = item.0.as_ref().map_or_else(
                || paint(c.color, Style::Grey, "undefined"),
                |item| inspect::quoted(item, c.color),
            );
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
                    .and_then(|data| table::render(data, columns.0.as_ref(), c.color));
                let text = drawn.unwrap_or_else(|| {
                    data.0.as_ref().map_or_else(
                        || paint(c.color, Style::Grey, "undefined"),
                        |v| inspect::line(std::slice::from_ref(v), c.color),
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
        written_with_color(source, false)
    }

    /// As [`lines_written_by`], with color switched on or off.
    fn written_with_color(source: &str, color: bool) -> Vec<(Level, String)> {
        let written = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&written);
        let output: Output = Rc::new(move |level, message| {
            sink.borrow_mut().push((level, message.to_owned()));
        });

        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            install(&ctx, &output, color).unwrap();
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
            [
                "1970-01-01T00:00:00.000Z",
                "Map(1) { 1 => 2 }",
                "Set(1) { 1 }",
                "Error: x",
                "{}"
            ]
        );
    }

    #[test]
    fn table_puts_non_tabular_rows_in_the_values_column() {
        let drawn = &texts("console.table([new Date(0), new Error('x')]);")[0];

        assert!(drawn.contains("Values"), "{drawn}");
        assert!(
            drawn.contains("│ 0       │ 1970-01-01T00:00:00.000Z "),
            "{drawn}"
        );
        assert!(drawn.contains("Error: x"), "{drawn}");
        assert!(drawn.contains(r"Error: x\n    at "), "{drawn}");
        assert_eq!(drawn.lines().count(), 6, "{drawn}");
    }

    #[test]
    fn table_shows_date_regexp_map_and_set_rows_as_one_line_values() {
        let drawn = &texts(
            "console.table([new Date(0), new Date(NaN), /a\\nb/g, new Map([['k', { v: 'x' }]]), new Set([1, 'y'])]);",
        )[0];

        assert_eq!(
            drawn,
            "┌─────────┬──────────────────────────────┐\n\
             │ (index) │ Values                       │\n\
             ├─────────┼──────────────────────────────┤\n\
             │ 0       │ 1970-01-01T00:00:00.000Z     │\n\
             │ 1       │ Invalid Date                 │\n\
             │ 2       │ /a\\nb/g                      │\n\
             │ 3       │ Map(1) { 'k' => { v: 'x' } } │\n\
             │ 4       │ Set(2) { 1, 'y' }            │\n\
             └─────────┴──────────────────────────────┘"
        );
    }

    #[test]
    fn table_cells_hold_the_four_types_inspected() {
        let drawn =
            &texts("console.table([{ d: new Date(0), m: new Map([[1, 2]]), s: new Set() }]);")[0];

        assert!(drawn.contains("│ 1970-01-01T00:00:00.000Z │"), "{drawn}");
        assert!(drawn.contains("│ Map(1) { 1 => 2 } │"), "{drawn}");
        assert!(drawn.contains("│ Set(0) {} │"), "{drawn}");
    }

    #[test]
    fn table_cells_escape_control_characters_inside_a_collection() {
        let drawn = &texts("console.table([{ m: new Map([['a\\nb', 'c\\td']]) }]);")[0];

        assert_eq!(drawn.lines().count(), 5, "{drawn}");
        assert!(drawn.contains(r"Map(1) { 'a\nb' => 'c\td' }"), "{drawn}");
    }

    #[test]
    fn dir_dirxml_and_the_specifiers_print_the_four_types() {
        assert_eq!(
            texts(
                "console.dir(new Map([['a', new Set(['b'])]])); console.dirxml(new Date(0)); \
                 console.log('%o|%O', /x/g, new Set([1])); \
                 console.log(new Map([[1, 2]]), new Date(NaN));"
            ),
            [
                "Map(1) { 'a' => Set(1) { 'b' } }",
                "1970-01-01T00:00:00.000Z",
                "/x/g|Set(1) { 1 }",
                "Map(1) { 1 => 2 } Invalid Date",
            ]
        );
    }

    #[test]
    fn an_error_property_holding_the_types_prints_them() {
        assert_eq!(
            texts("console.log({ e: Object.assign(new Error('x'), { when: new Date(0) }), m: new Map() });")[0]
                .lines()
                .next()
                .unwrap(),
            "{ e: Error: x"
        );
        assert_eq!(
            texts("const e = new Error('x'); e.stack = ''; console.log([e, new Set([1])]);"),
            ["[ Error: x, Set(1) { 1 } ]"]
        );
    }

    #[test]
    fn hostile_values_print_without_leaving_an_exception() {
        assert_eq!(
            texts(
                "const hostile = new Date(0); hostile.valueOf = () => { throw 1; }; \
                 hostile.toISOString = () => { throw 2; }; console.log(hostile); \
                 const m = new Map([[1, 2]]); m.entries = () => { throw 3; }; \
                 m[Symbol.iterator] = () => { throw 4; }; console.log(m); \
                 console.log(new Proxy(new Map(), {})); \
                 const r = Proxy.revocable(new Set(), {}); r.revoke(); console.log(r.proxy); \
                 console.log(new (class S extends Set {})([1])); \
                 const c = new Map(); c.set('self', c); console.log(c); \
                 console.log(new Set(Array.from({ length: 100000 }, (_, i) => i)).size); \
                 try { null.x; } catch (e) { console.log('caught'); }"
            ),
            [
                "1970-01-01T00:00:00.000Z",
                "Map(1) { 1 => 2 }",
                "Map(0) {}",
                "<Revoked Proxy>",
                "Set(1) { 1 }",
                "Map(1) { 'self' => Map(1) { 'self' => Map(1) { 'self' => [Map] } } }",
                "100000",
                "caught"
            ]
        );
    }

    #[test]
    fn a_colored_table_of_the_types_has_the_layout_of_the_plain_one() {
        drawn_both_ways("[new Date(0), /x/g, new Map([['日本', [1]]]), new Set(['a\\nb'])]");
    }

    #[test]
    fn color_off_prints_the_types_without_an_escape_sequence() {
        let source = "console.log(new Date(0), new Date(NaN), /a/g, new Map([[1, 'x']]), new Set()); \
                      console.dir(new Map()); console.table([new Date(0)]);";

        assert!(texts(source).iter().all(|t| !t.contains('\x1b')));
        assert_eq!(
            colored_texts(source)
                .iter()
                .map(|t| without_known_codes(t))
                .collect::<Vec<_>>(),
            texts(source)
        );
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
    fn accessors_and_a_failing_stack_in_any_method_leave_nothing_pending() {
        assert_eq!(
            texts(
                "console.log({ get a() { throw 1; } }); \
                 console.log([Object.defineProperty([1], 0, { get() { throw 1; } })]); \
                 console.log(Object.defineProperty(new Error('e'), 'stack', { get() { throw 1; } })); \
                 console.dir({ get a() { throw 1; } }); \
                 try { null.x; } catch (e) { console.log('caught'); }"
            )
            .iter()
            .map(|t| t.lines().next().unwrap().to_owned())
            .collect::<Vec<_>>(),
            [
                "{ a: [Getter] }",
                "[ [ [Getter] ] ]",
                "Error: e",
                "{ a: [Getter] }",
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

    #[test]
    fn install_fails_in_a_realm_without_the_intrinsics_and_leaves_nothing_pending() {
        let runtime = Runtime::new().unwrap();
        let context = Context::custom::<rquickjs::context::intrinsic::Eval>(&runtime).unwrap();
        context.with(|ctx| {
            let output: Output = Rc::new(|_, _| {});

            assert!(install(&ctx, &output, false).is_err());
            let thrown = format!("{:?}", ctx.catch());
            assert!(thrown.contains("Date is not defined"), "{thrown}");
            assert!(!ctx.has_exception());
        });
    }

    #[test]
    fn contexts_in_one_runtime_share_the_pins_after_the_first_is_dropped() {
        let written = Rc::new(RefCell::new(Vec::new()));
        let output: Output = {
            let sink = Rc::clone(&written);
            Rc::new(move |_, message| sink.borrow_mut().push(message.to_owned()))
        };
        let runtime = Runtime::new().unwrap();
        let first = Context::full(&runtime).unwrap();
        let second = Context::full(&runtime).unwrap();
        first.with(|ctx| install(&ctx, &output, false).unwrap());
        second.with(|ctx| install(&ctx, &output, false).unwrap());
        drop(first);
        runtime.run_gc();

        second.with(|ctx| {
            ctx.eval::<(), _>("console.log(new Date(0), new Map([[1, 2]]), /x/g, new Set([3]));")
                .unwrap();
            assert!(!ctx.has_exception());
        });
        assert_eq!(
            written.take(),
            ["1970-01-01T00:00:00.000Z Map(1) { 1 => 2 } /x/g Set(1) { 3 }"]
        );
    }

    fn colored_texts(source: &str) -> Vec<String> {
        written_with_color(source, true)
            .into_iter()
            .map(|(_, t)| t)
            .collect()
    }

    /// `text` without ANSI color sequences.
    fn strip(text: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(at) = rest.find("\x1b[") {
            out.push_str(&rest[..at]);
            let end = rest[at..].find('m').unwrap();
            rest = &rest[at + end + 1..];
        }
        out.push_str(rest);
        out
    }

    const GUIDE: &str = "\x1b[90m│ \x1b[39m";

    fn os(text: &str) -> &OsStr {
        OsStr::new(text)
    }

    #[test]
    fn a_non_empty_no_color_turns_color_off_whatever_else_is_set() {
        for force in [
            None,
            Some(os("")),
            Some(os("1")),
            Some(os("0")),
            Some(os("true")),
        ] {
            for terminal in [false, true] {
                assert!(!color_enabled(Some(os("1")), force, terminal));
                assert!(!color_enabled(Some(os("anything")), force, terminal));
            }
        }
    }

    #[test]
    fn an_empty_no_color_is_ignored() {
        assert!(color_enabled(Some(os("")), None, true));
        assert!(!color_enabled(Some(os("")), None, false));
        assert!(color_enabled(Some(os("")), Some(os("1")), false));
    }

    #[test]
    fn force_color_zero_or_false_turns_color_off_on_a_terminal() {
        assert!(!color_enabled(None, Some(os("0")), true));
        assert!(!color_enabled(None, Some(os("false")), true));
        assert!(!color_enabled(None, Some(os("0")), false));
        assert!(!color_enabled(None, Some(os("false")), false));
    }

    #[test]
    fn any_other_non_empty_force_color_turns_color_on_without_a_terminal() {
        for value in ["1", "2", "3", "true", "yes", "FALSE", "00"] {
            assert!(color_enabled(None, Some(os(value)), false), "{value}");
            assert!(color_enabled(None, Some(os(value)), true), "{value}");
        }
    }

    #[test]
    fn an_empty_force_color_falls_through_to_the_terminal() {
        assert!(color_enabled(None, Some(os("")), true));
        assert!(!color_enabled(None, Some(os("")), false));
    }

    #[test]
    fn without_either_variable_the_terminal_decides() {
        assert!(color_enabled(None, None, true));
        assert!(!color_enabled(None, None, false));
    }

    #[test]
    fn values_are_colored_by_type_in_a_log_line() {
        assert_eq!(
            colored_texts("console.log('s', 1, true, null, undefined, { k: 'v' });"),
            [
                "s \x1b[33m1\x1b[39m \x1b[33mtrue\x1b[39m \x1b[1mnull\x1b[22m \x1b[90mundefined\x1b[39m { k: \x1b[32m'v'\x1b[39m }"
            ]
        );
    }

    #[test]
    fn each_level_symbol_is_colored_and_its_body_is_not() {
        assert_eq!(
            colored_texts(
                "console.error('e'); console.warn('w'); console.info('i'); \
                 console.log('l'); console.dirxml('x');"
            ),
            [
                "\x1b[31m✖\x1b[39m e",
                "\x1b[33m⚠\x1b[39m w",
                "\x1b[34mℹ\x1b[39m i",
                "l",
                "x"
            ]
        );
    }

    #[test]
    fn a_failed_assert_has_a_red_symbol() {
        assert_eq!(
            colored_texts("console.assert(false, 'bad');"),
            ["\x1b[31m✖\x1b[39m Assertion failed: bad"]
        );
    }

    #[test]
    fn a_warning_from_a_counter_or_timer_has_a_yellow_symbol() {
        assert_eq!(
            colored_texts("console.countReset('q'); console.timeEnd('q');"),
            [
                "\x1b[33m⚠\x1b[39m Count for 'q' does not exist",
                "\x1b[33m⚠\x1b[39m Timer 'q' does not exist"
            ]
        );
    }

    #[test]
    fn a_debug_line_is_dim_as_a_whole_without_value_colors() {
        assert_eq!(
            colored_texts("console.debug('d', 1, { a: 'b' });"),
            ["\x1b[90md 1 { a: 'b' }\x1b[39m"]
        );
    }

    #[test]
    fn every_line_of_a_multi_line_debug_body_is_dim() {
        assert_eq!(
            colored_texts("console.debug('a\\nb');"),
            ["\x1b[90ma\x1b[39m\n\x1b[90mb\x1b[39m"]
        );
    }

    #[test]
    fn group_guides_are_dim_at_every_depth() {
        assert_eq!(
            colored_texts(
                "console.group('a'); console.group('b'); console.log('x'); console.warn('w');"
            ),
            [
                "▼ a".to_owned(),
                format!("{GUIDE}▼ b"),
                format!("{GUIDE}{GUIDE}x"),
                format!("{GUIDE}{GUIDE}\x1b[33m⚠\x1b[39m w"),
            ]
        );
    }

    #[test]
    fn a_multi_line_body_lines_up_with_color_on() {
        assert_eq!(
            colored_texts("console.group('g'); console.error('a\\nb');"),
            [
                "▼ g".to_owned(),
                format!("{GUIDE}\x1b[31m✖\x1b[39m a\n{GUIDE}  b"),
            ]
        );
    }

    #[test]
    fn an_empty_line_keeps_no_trailing_space_with_color_on() {
        assert_eq!(
            colored_texts("console.group(''); console.log(''); console.error('');"),
            [
                "▼".to_owned(),
                "\x1b[90m│\x1b[39m".to_owned(),
                format!("{GUIDE}\x1b[31m✖\x1b[39m"),
            ]
        );
    }

    #[test]
    fn the_first_line_of_an_error_is_plain_and_its_frames_are_dim() {
        let text = &colored_texts("console.log(new Error('boom'));")[0];
        let mut lines = text.lines();

        assert_eq!(lines.next(), Some("Error: boom"));
        for frame in lines {
            assert!(
                frame.starts_with("\x1b[90m") && frame.ends_with("\x1b[39m"),
                "{frame:?}"
            );
        }
    }

    #[test]
    fn trace_keeps_its_first_line_plain_and_dims_each_frame() {
        let text = &colored_texts(
            "function inner() { console.trace('hi', 1); }\nfunction outer() { inner(); }\nouter();",
        )[0];
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(lines[0], "Trace: hi \x1b[33m1\x1b[39m");
        assert!(lines.len() >= 3, "{lines:?}");
        for frame in &lines[1..] {
            assert!(
                frame.starts_with("\x1b[90m") && frame.ends_with("\x1b[39m"),
                "{frame:?}"
            );
        }
    }

    #[test]
    fn trace_inside_a_group_dims_guides_and_frames() {
        let text = &colored_texts("console.group('g'); console.trace();")[1];

        for (index, line) in text.lines().enumerate() {
            assert!(line.starts_with(GUIDE), "{line:?}");
            let rest = &line[GUIDE.len()..];
            assert_eq!(rest.starts_with("\x1b[90m"), index > 0, "{line:?}");
        }
    }

    #[test]
    fn dir_and_the_table_fallback_color_their_value() {
        assert_eq!(
            colored_texts("console.dir('s'); console.dir(); console.table(5); console.table();"),
            [
                "\x1b[32m's'\x1b[39m",
                "\x1b[90mundefined\x1b[39m",
                "\x1b[33m5\x1b[39m",
                "\x1b[90mundefined\x1b[39m"
            ]
        );
    }

    #[test]
    fn time_log_colors_its_extra_data() {
        let lines = colored_texts("console.time('t'); console.timeLog('t', 2);");

        assert!(lines[0].ends_with(" \x1b[33m2\x1b[39m"), "{}", lines[0]);
        assert!(lines[0].starts_with("t: "), "{}", lines[0]);
    }

    #[test]
    fn a_group_label_and_assert_data_are_colored() {
        assert_eq!(
            colored_texts("console.group('a', 1); console.assert(false, 'm', 2);"),
            [
                "▼ a \x1b[33m1\x1b[39m".to_owned(),
                format!("{GUIDE}\x1b[31m✖\x1b[39m Assertion failed: m \x1b[33m2\x1b[39m"),
            ]
        );
    }

    #[test]
    fn format_specifiers_color_inspected_arguments() {
        assert_eq!(
            colored_texts("console.log('%o|%s', 'q', 'r', 3);"),
            ["\x1b[32m'q'\x1b[39m|r \x1b[33m3\x1b[39m"]
        );
    }

    #[test]
    fn a_colored_table_has_the_layout_of_the_plain_one() {
        let source = "console.group('g'); console.table([{ a: 1, b: 'x' }, { a: [1, 'y'], c: null }, 5, 'z']);";
        let plain = texts(source);
        let colored = colored_texts(source);

        assert_eq!(colored.len(), plain.len());
        assert_eq!(strip(&colored[1]), plain[1]);
        assert!(colored[1].contains("\x1b[33m1\x1b[39m"), "{}", colored[1]);
        assert!(colored[1].contains("\x1b[32m'x'\x1b[39m"), "{}", colored[1]);
    }

    #[test]
    fn a_colored_table_cell_is_padded_by_its_visible_width() {
        let drawn = &colored_texts("console.table([{ k: 'ab' }, { k: 1 }]);")[0];

        assert_eq!(
            drawn.lines().nth(3).unwrap(),
            "│ 0       │ \x1b[32m'ab'\x1b[39m │"
        );
        assert_eq!(
            drawn.lines().nth(4).unwrap(),
            "│ 1       │ \x1b[33m1\x1b[39m    │"
        );
    }

    #[test]
    fn a_colored_table_leaves_headers_and_rules_uncolored() {
        let drawn = &colored_texts("console.table([{ a: 1 }]);")[0];
        let lines: Vec<&str> = drawn.lines().collect();

        assert_eq!(lines[0], "┌─────────┬───┐");
        assert_eq!(lines[1], "│ (index) │ a │");
        assert_eq!(lines[2], "├─────────┼───┤");
    }

    #[test]
    fn a_colored_table_escapes_control_characters_like_the_plain_one() {
        let source = r"console.table([{ k: ['a\nb', 1] }]);";

        assert_eq!(strip(&colored_texts(source)[0]), texts(source)[0]);
    }

    #[test]
    fn color_off_writes_no_escape_sequences_for_any_method() {
        let source = "console.group('g'); console.log('s', 1, null, undefined, { a: ['b'] }); \
             console.debug('d'); console.info('i'); console.warn('w'); console.error(new Error('e')); \
             console.assert(false, 'x'); console.trace('t'); console.dir('q'); \
             console.table([{ a: 1 }, 'v']); console.count(); console.countReset('z'); \
             console.groupEnd(); console.log(function f() {}, Symbol('s'), 10n);";

        assert!(texts(source).iter().all(|t| !t.contains('\x1b')));
    }

    #[test]
    fn color_on_adds_only_sequences_to_the_plain_output() {
        let source = "console.group('g'); console.log('s', 1, null, undefined, { a: ['b'] }); \
             console.debug('d'); console.info('i'); console.warn('w'); \
             console.error('e\\nf'); console.assert(false, 'x'); console.dir('q'); \
             console.table([{ a: 1 }, 'v']); console.count(); console.countReset('z'); \
             console.groupEnd(); console.log(function f() {}, Symbol('s'), 10n);";
        let plain = texts(source);
        let colored: Vec<String> = colored_texts(source).iter().map(|t| strip(t)).collect();

        assert_eq!(colored, plain);
    }

    #[test]
    fn color_does_not_change_the_levels() {
        let source = "console.trace('a'); console.debug('a'); console.log('a'); \
             console.info('a'); console.warn('a'); console.error('a');";
        let levels = |color| -> Vec<Level> {
            written_with_color(source, color)
                .iter()
                .map(|(level, _)| *level)
                .collect()
        };

        assert_eq!(levels(true), levels(false));
    }

    /// Whether every color sequence on `line` is closed on `line`.
    fn balanced(line: &str) -> bool {
        let mut open = 0usize;
        let mut close = 0usize;
        let mut rest = line;
        while let Some(at) = rest.find("\x1b[") {
            let end = at + rest[at..].find('m').unwrap();
            if matches!(&rest[at + 2..end], "39" | "22") {
                close += 1;
            } else {
                open += 1;
            }
            rest = &rest[end + 1..];
        }
        open == close
    }

    /// `text` without the color sequences this crate paints.
    fn without_known_codes(text: &str) -> String {
        let mut out = text.to_owned();
        for code in [31, 32, 33, 34, 35, 36, 1, 90, 39, 22] {
            out = out.replace(&format!("\x1b[{code}m"), "");
        }
        out
    }

    #[test]
    fn color_never_crosses_a_line_break_in_a_nested_string() {
        assert_eq!(
            colored_texts("console.log(['a\\nb']);"),
            ["[ \x1b[32m'a\x1b[39m\n\x1b[32mb'\x1b[39m ]"]
        );
    }

    #[test]
    fn color_never_crosses_a_line_break_inside_nested_groups() {
        let lines = colored_texts(
            "console.group('a'); console.group('b'); console.warn(['x\\ny'], Symbol('p\\nq'));",
        );

        assert_eq!(
            lines[2],
            format!(
                "{GUIDE}{GUIDE}\x1b[33m⚠\x1b[39m [ \x1b[32m'x\x1b[39m\n\
                 {GUIDE}{GUIDE}  \x1b[32my'\x1b[39m ] \x1b[32mSymbol(p\x1b[39m\n\
                 {GUIDE}{GUIDE}  \x1b[32mq)\x1b[39m"
            )
        );
        for line in lines[2].lines() {
            assert!(balanced(line), "{line:?}");
        }
    }

    #[test]
    fn a_multi_line_error_message_is_balanced_on_every_line() {
        let text = &colored_texts("console.error(new Error('x\\ny'));")[0];
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(lines[0], "\x1b[31m✖\x1b[39m Error: x");
        assert_eq!(lines[1], "  y");
        assert!(lines.len() > 2);
        assert!(lines.iter().all(|line| balanced(line)), "{lines:?}");
    }

    #[test]
    fn a_debug_line_with_a_multi_line_value_is_dim_on_each_line() {
        assert_eq!(
            colored_texts("console.group('g'); console.debug({ a: 'x\\ny' });")[1],
            format!("{GUIDE}\x1b[90m{{ a: 'x\x1b[39m\n{GUIDE}\x1b[90my' }}\x1b[39m")
        );
    }

    #[test]
    fn every_color_in_every_method_is_closed_on_its_own_line() {
        let source = "console.group('g\\nh'); console.log(['a\\nb'], Symbol('s\\nt')); \
             console.info({ 'k\\nz': 'v\\nw' }); console.error(new Error('e\\nf')); \
             console.trace('t\\nu'); console.debug('d\\ne'); console.assert(false, ['q\\nr']); \
             console.dir(['m\\nn']); console.table([['c\\nd', 1]]);";

        for text in colored_texts(source) {
            for line in text.lines() {
                assert!(balanced(line), "{line:?}");
            }
        }
    }

    /// The colored and the plain drawing of `console.table(args)`, both
    /// checked to be aligned and free of live sequences other than ours.
    fn drawn_both_ways(args: &str) -> (String, String) {
        let source = format!("console.table({args});");
        let plain = texts(&source).remove(0);
        let colored = colored_texts(&source).remove(0);
        let bare = without_known_codes(&colored);

        assert!(!bare.contains('\x1b'), "{colored:?}");
        assert_eq!(bare, plain);
        let widths: Vec<usize> = plain.lines().map(UnicodeWidthStr::width).collect();
        assert!(widths.windows(2).all(|w| w[0] == w[1]), "{plain}");
        assert!(colored.lines().all(balanced), "{colored:?}");
        (plain, colored)
    }

    #[test]
    fn a_table_cell_holding_an_escape_sequence_shows_it_as_text() {
        let (plain, colored) = drawn_both_ways(r"[{ k: ['\x1b[1mx'] }]");

        assert!(plain.contains(r"[ '\x1b[1mx' ]"), "{plain}");
        assert!(colored.contains(r"\x1b[1mx"), "{colored}");
    }

    #[test]
    fn a_table_cell_holding_a_nested_color_code_stays_inert() {
        let (plain, _) = drawn_both_ways(r"[{ k: { v: '\x1b[31m' }, 'z\x1b[0m': 1 }]");

        assert!(plain.contains(r"'\x1b[31m'"), "{plain}");
        assert!(plain.contains(r"z\x1b[0m"), "{plain}");
    }

    #[test]
    fn a_table_cell_with_a_tab_or_newline_stays_on_one_line() {
        let (plain, _) = drawn_both_ways(r"[{ k: ['a\tb', 'c\nd'], 'e\tf': 1 }, 'x\ty']");

        assert_eq!(plain.lines().count(), 6, "{plain}");
        assert!(plain.contains(r"'a\tb'"), "{plain}");
        assert!(plain.contains(r"'c\nd'"), "{plain}");
        assert!(plain.contains(r"e\tf"), "{plain}");
    }

    #[test]
    fn a_table_with_a_wide_and_a_colored_cell_stays_aligned() {
        drawn_both_ways(r"[{ a: '日本', b: [1, 'x'], c: null, d: undefined, e: () => 1 }]");
    }
}
