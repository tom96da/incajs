// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `globalThis.console` object: [`install`] defines it, and [`Output`]
//! receives what it prints.

mod format;
mod state;
mod table;

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{
    Coerced, Ctx, Function, Object, Result as JsResult, Value,
    function::{IntoJsFunc, Opt, Rest},
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
    format::substitute(ctx, &data, color)
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
/// The `NO_COLOR` and `FORCE_COLOR` conventions are the usual source of it. Install before evaluating
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

/// `console.assert`: an error line when the condition is falsy.
fn assert<'js>(c: Rc<Console>) -> impl Fn(Ctx<'js>, Opt<Value<'js>>, Rest<Value<'js>>) {
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

/// `console.count`: increments the label's counter and prints it.
fn count(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>) {
    move |label| {
        let label = label_of(&label);
        let count = c.state.borrow_mut().count(&label);
        c.write(Level::Log, &format!("{label}: {count}"));
    }
}

/// `console.countReset`: zeroes the label's counter, warning when it has none.
fn count_reset(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>) {
    move |label| {
        let label = label_of(&label);
        if !c.state.borrow_mut().reset_count(&label) {
            c.warn(&format!("Count for '{label}' does not exist"));
        }
    }
}

/// `console.time`: starts the label's timer, warning when it is running.
fn time(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>) {
    move |label| {
        let label = label_of(&label);
        if !c.state.borrow_mut().start_timer(&label) {
            c.warn(&format!("Timer '{label}' already exists"));
        }
    }
}

/// `console.timeLog`: prints the label's elapsed time and the extra data.
fn time_log(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>, Rest<Value<'_>>) {
    move |label, data| c.time_log(&label_of(&label), &data.0, false)
}

/// `console.timeEnd`: prints the label's elapsed time and stops the timer.
fn time_end(c: Rc<Console>) -> impl Fn(Opt<Value<'_>>) {
    move |label| c.time_log(&label_of(&label), &[], true)
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
mod tests;
