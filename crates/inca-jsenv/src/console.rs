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
use crate::sourcemap::remap_stack;
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
    let stack = remap_stack(ctx, &stack);
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
mod tests;
