// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `globalThis.console`.

use std::rc::Rc;

use rquickjs::{Ctx, Function, Object, Result as JsResult, Value, function::Rest};

use crate::inspect;

/// Which `console` method produced a line. Carried through to [`Output`] so a
/// reader can tell an error from a trace; nothing here filters on it.
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
    /// The method name this level came from.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Level::Trace => "trace",
            Level::Debug => "debug",
            Level::Log => "log",
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// Where a formatted `console` line goes.
///
/// Taken as an argument rather than chosen here: an embedder may already be
/// using stdout for something a stray line would corrupt, and a test wants to
/// read back what was written.
pub type Output = Rc<dyn Fn(Level, &str)>;

/// An [`Output`] writing `level: message` to stderr.
///
/// Deliberately not routed through the `log` crate. A logger's default filter
/// discards anything below its threshold, and an application's own `console`
/// output disappearing is the problem `console` exists to solve.
#[must_use]
pub fn to_stderr() -> Output {
    Rc::new(|level, message| eprintln!("{}: {message}", level.label()))
}

/// Installs `globalThis.console`, writing through `output`.
///
/// Every method formats its arguments the same way and differs only in the
/// [`Level`] it reports. Install this before evaluating anything: a script
/// that logs during its own top-level evaluation has no other chance.
///
/// # Errors
///
/// Returns an error if defining `console` or any of its methods fails.
pub fn install(ctx: &Ctx<'_>, output: &Output) -> JsResult<()> {
    let console = Object::new(ctx.clone())?;

    for level in [
        Level::Trace,
        Level::Debug,
        Level::Log,
        Level::Info,
        Level::Warn,
        Level::Error,
    ] {
        let output = Rc::clone(output);
        console.set(
            level.label(),
            Function::new(ctx.clone(), move |values: Rest<Value<'_>>| {
                output(level, &inspect::line(&values.0));
            })?,
        )?;
    }

    ctx.globals().set("console", console)?;
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

    #[test]
    fn console_is_reachable_before_anything_else_runs() {
        let written = lines_written_by("typeof console === 'object' && console.log('present');");

        assert_eq!(written[0].1, "present");
    }
}
