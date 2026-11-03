// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `globalThis.console` object: [`install`] defines it, and [`Output`]
//! receives what it prints.

mod assert;
mod counters;
mod format;
mod state;
mod table;

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{
    Ctx, Function, Object, Result as JsResult, Value,
    function::{IntoJsFunc, Opt, Rest},
};
use unicode_width::UnicodeWidthStr;

use crate::inspect;
use crate::paint::{Style, paint};
use crate::sourcemap::remap_stack;
use assert::assert;
use counters::{count, count_reset, time, time_end, time_log};
use state::State;

/// Starts every line, once per open group.
const GROUP_INDENT: &str = "│ ";

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

struct Console {
    output: Output,
    state: RefCell<State>,
    color: bool,
}

impl Console {
    /// A debug line is dimmed whole, so its values stay uncolored.
    fn format(&self, ctx: &Ctx<'_>, level: Level, values: &[Value<'_>]) -> String {
        format::substitute(ctx, values, self.color && level != Level::Debug)
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

/// The frames of the stack that called `console.trace`. `QuickJS` lists no
/// frame for a native function, so skipping this evaluation's own leaves the
/// caller on top.
fn caller_stack(ctx: &Ctx<'_>) -> Vec<String> {
    let stack =
        inspect::settled(ctx, ctx.eval::<String, _>("new Error().stack")).unwrap_or_default();
    let stack = remap_stack(ctx, &stack);
    stack
        .lines()
        .map(str::trim_end)
        .skip(1)
        .map(str::to_owned)
        .collect()
}

/// Defines `globalThis.console` and writes each formatted line through `output`.
///
/// `output` receives the level and the formatted line. With `color` set, the
/// lines carry ANSI color sequences. Install before evaluating any script, so
/// top-level logging works.
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

    let methods = Methods {
        ctx,
        console: &console,
        shared: &shared,
    };
    install_methods(&methods)?;

    ctx.globals().set("console", console)?;
    Ok(())
}

/// Registers methods on the console object, handing each one its own clone of the shared state.
struct Methods<'a, 'js> {
    ctx: &'a Ctx<'js>,
    console: &'a Object<'js>,
    shared: &'a Rc<Console>,
}

impl<'js> Methods<'_, 'js> {
    /// Sets the method `name`, built by `make` from a clone of the shared state.
    fn set<P, F>(&self, name: &str, make: impl FnOnce(Rc<Console>) -> F) -> JsResult<()>
    where
        F: IntoJsFunc<'js, P> + 'js,
    {
        self.console.set(
            name,
            Function::new(self.ctx.clone(), make(Rc::clone(self.shared)))?,
        )
    }
}

/// Registers every console method.
fn install_methods(m: &Methods<'_, '_>) -> JsResult<()> {
    // output
    for (name, level) in [
        ("debug", Level::Debug),
        ("log", Level::Log),
        ("dirxml", Level::Log),
        ("info", Level::Info),
        ("warn", Level::Warn),
        ("error", Level::Error),
    ] {
        m.set(name, |c| print_at(c, level))?;
    }
    m.set("trace", trace)?;

    // counting
    m.set("assert", assert)?;
    m.set("count", count)?;
    m.set("countReset", count_reset)?;

    // timing
    m.set("time", time)?;
    m.set("timeLog", time_log)?;
    m.set("timeEnd", time_end)?;

    // structure
    m.set("group", group)?;
    m.set("groupCollapsed", group_collapsed)?;
    m.set("groupEnd", group_end)?;
    m.set("clear", clear)?;
    m.set("dir", dir)?;
    m.set("table", table)
}

/// Builds a console method that formats its arguments and prints them at `level`.
fn print_at(c: Rc<Console>, level: Level) -> impl Fn(Ctx<'_>, Rest<Value<'_>>) {
    move |ctx, values| c.write(level, &c.format(&ctx, level, &values.0))
}

/// `console.trace`: "Trace", the formatted arguments and the caller's frames.
fn trace(c: Rc<Console>) -> impl Fn(Ctx<'_>, Rest<Value<'_>>) {
    move |ctx, values| {
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
    }
}

/// `console.group`: prints the label behind an open marker and indents what follows.
fn group(c: Rc<Console>) -> impl Fn(Ctx<'_>, Rest<Value<'_>>) {
    move |ctx, label| c.group(&ctx, "▼", &label.0)
}

/// `console.groupCollapsed`: `group` with the collapsed marker.
fn group_collapsed(c: Rc<Console>) -> impl Fn(Ctx<'_>, Rest<Value<'_>>) {
    move |ctx, label| c.group(&ctx, "▶", &label.0)
}

/// `console.groupEnd`: closes the innermost group.
fn group_end(c: Rc<Console>) -> impl Fn() {
    move || c.state.borrow_mut().close_group()
}

/// `console.clear`: closes every group.
fn clear(c: Rc<Console>) -> impl Fn() {
    move || c.state.borrow_mut().close_all_groups()
}

/// `console.dir`: prints the value with its strings quoted.
fn dir(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>) {
    move |item| {
        let text = item.0.as_ref().map_or_else(
            || paint(c.color, Style::Grey, "undefined"),
            |item| inspect::quoted(item, c.color),
        );
        c.write(Level::Log, &text);
    }
}

/// `console.table`: draws the data as a table, or prints it as `log` does.
fn table(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>, Opt<Value<'_>>) {
    move |data, columns| {
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
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rquickjs::{Context, Runtime};

    use super::*;
    use crate::tests::{lines_written_by, texts, without_elapsed, written_by};

    #[test]
    fn every_method_writes_at_its_own_level() {
        let written = lines_written_by(
            r"
            console.trace('a'); console.debug('a'); console.log('a');
            console.info('a'); console.warn('a'); console.error('a');
            ",
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
    fn logging_nothing_writes_an_empty_line() {
        let written = lines_written_by("console.log();");

        assert_eq!(written.len(), 1);
        assert_eq!(written[0].1, "");
    }

    #[test]
    fn symbols_mark_error_warn_and_info_only() {
        assert_eq!(
            texts(
                r"
                console.error('e'); console.warn('w'); console.info('i');
                console.log('l'); console.debug('d'); console.dirxml('x');
                "
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
                r"
                console.group('a'); console.log('1'); console.group('b'); console.log('2');
                console.groupEnd(); console.log('3'); console.groupEnd(); console.log('4');
                "
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
    fn trace_frames_read_at_their_source() {
        let body = crate::sourcemap::mapped_body("console.trace('hi');");
        let written = written_by(false, |ctx| {
            let f = crate::compile_async_function(ctx, &[], &body, "virtual.js").unwrap();
            ctx.globals().set("f", f).unwrap();
            ctx.eval::<(), _>("f();").unwrap();
        });

        let text = &written[0].1;
        assert!(text.contains("src/app.ts:12:5"), "{text}");
        assert!(!text.contains("virtual.js:"), "{text}");
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
                r"
                console.group(''); console.log(''); console.log('a\n\nb');
                console.error(''); console.error('a\n\nb'); console.log();
                "
            ),
            ["▼", "│", "│ a\n│\n│ b", "│ ✖", "│ ✖ a\n│\n│   b", "│"]
        );
        assert_eq!(texts("console.log('');"), [""]);
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
    fn a_context_keeps_printing_after_another_context_is_dropped() {
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
    fn every_formatting_method_substitutes_specifiers() {
        let written = lines_written_by(
            r"
            console.log('%s=%d', 'a', 1); console.debug('%s', 'x', 'y');
            console.info('%o', 'i'); console.warn('%f', '1.5');
            console.error('%s', Symbol('e')); console.dirxml('%i', '7.9');
            ",
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
    fn dir_table_and_the_counters_do_not_format() {
        assert_eq!(
            texts(
                r"
                console.dir('%s', 'x'); console.table('%s'); console.count('%s');
                console.countReset('%d'); console.timeEnd('%i');
                "
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
}
