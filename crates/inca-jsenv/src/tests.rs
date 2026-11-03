// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the tests of several modules.

#![allow(clippy::unwrap_used)]

use std::{
    cell::RefCell,
    env, fs,
    ops::Deref,
    path::{Path, PathBuf},
    rc::Rc,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rquickjs::{Context, Ctx, Runtime, Value};
use unicode_width::UnicodeWidthStr;

use crate::Engine;
use crate::console::{Level, Output, install};
use crate::inspect::{line, pin, quoted_escaped};
use crate::paint::Style;
use crate::sourcemap::{register, remap_stack};

/// Runs `body` in a fresh runtime with a full context.
pub(crate) fn with_realm<R>(body: impl FnOnce(Ctx<'_>) -> R) -> R {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    context.with(body)
}

/// A directory under the OS temp root, unique per process, test name and
/// thread, removed on drop.
pub(crate) struct ScratchDir(pub(crate) PathBuf);

impl ScratchDir {
    pub(crate) fn new(name: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "inca-jsenv-test-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Deref for ScratchDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `text` without the sequences of every [`Style`]. Other escapes stay, so a
/// stray one is still visible.
pub(crate) fn strip(text: &str) -> String {
    let mut out = text.to_owned();
    for style in [
        Style::Black,
        Style::Red,
        Style::Green,
        Style::Yellow,
        Style::Blue,
        Style::Magenta,
        Style::Cyan,
        Style::White,
        Style::Bold,
        Style::Grey,
    ] {
        let (open, close) = style.codes();
        out = out
            .replace(&format!("\x1b[{open}m"), "")
            .replace(&format!("\x1b[{close}m"), "");
    }
    out
}

/// The counters a leak moves, after a gc run.
pub(crate) fn counters(engine: &Engine) -> [i64; 5] {
    engine.runtime.run_gc();
    let usage = engine.runtime.memory_usage();
    [
        usage.atom_count,
        usage.str_count,
        usage.obj_count,
        usage.malloc_count,
        usage.memory_used_count,
    ]
}

/// The bytes of memory the engine holds after a gc run.
pub(crate) fn memory_size(engine: &Engine) -> i64 {
    engine.runtime.run_gc();
    engine.runtime.memory_usage().memory_used_size
}

/// Calls `each` once to warm up, then 10 000 times, and returns the counters before and after.
pub(crate) fn around(engine: &Engine, each: impl Fn(&Ctx<'_>)) -> ([i64; 5], [i64; 5]) {
    engine.with(|ctx| each(&ctx));
    let before = counters(engine);
    engine.with(|ctx| {
        for _ in 0..10_000 {
            each(&ctx);
        }
    });
    (before, counters(engine))
}

/// Evaluates `source` against a realm with `console` installed, and
/// returns every line it wrote.
pub(crate) fn lines_written_by(source: &str) -> Vec<(Level, String)> {
    written_with_color(source, false)
}

/// Runs `run` against a realm with `console` installed, and returns every
/// line it wrote.
pub(crate) fn written_by(color: bool, run: impl FnOnce(&Ctx<'_>)) -> Vec<(Level, String)> {
    let written = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&written);
    let output: Output = Rc::new(move |level, message| {
        sink.borrow_mut().push((level, message.to_owned()));
    });

    with_realm(|ctx| {
        install(&ctx, &output, color).unwrap();
        run(&ctx);
        assert!(!ctx.has_exception(), "a call left an exception pending");
    });

    written.take()
}

/// The `(level, text)` pairs `source` wrote, joined into one string per
/// line for compact comparison.
pub(crate) fn texts(source: &str) -> Vec<String> {
    lines_written_by(source)
        .into_iter()
        .map(|(_, t)| t)
        .collect()
}

/// Replaces the number in `label: 1.234ms` so a line can be compared.
pub(crate) fn without_elapsed(line: &str) -> String {
    let (label, rest) = line.split_once(": ").unwrap();
    let (number, tail) = rest.split_once("ms").unwrap();
    assert!(number.parse::<f64>().is_ok(), "{line}");
    assert_eq!(number.split_once('.').unwrap().1.len(), 3, "{line}");
    format!("{label}: Nms{tail}")
}

/// The colored and the plain drawing of `console.table(args)`, both
/// checked to be aligned and free of live sequences other than ours.
pub(crate) fn drawn_both_ways(args: &str) -> (String, String) {
    let source = format!("console.table({args});");
    let plain = texts(&source).remove(0);
    let colored = colored_texts(&source).remove(0);
    let bare = strip(&colored);

    assert!(!bare.contains('\x1b'), "{colored:?}");
    assert_eq!(bare, plain);
    let widths: Vec<usize> = plain.lines().map(UnicodeWidthStr::width).collect();
    assert!(widths.windows(2).all(|w| w[0] == w[1]), "{plain}");
    assert!(colored.lines().all(balanced), "{colored:?}");
    (plain, colored)
}

/// As [`lines_written_by`], with color switched on or off.
pub(crate) fn written_with_color(source: &str, color: bool) -> Vec<(Level, String)> {
    written_by(color, |ctx| ctx.eval::<(), _>(source).unwrap())
}

pub(crate) fn colored_texts(source: &str) -> Vec<String> {
    written_with_color(source, true)
        .into_iter()
        .map(|(_, t)| t)
        .collect()
}

/// Whether every color sequence on `line` is closed on `line`.
pub(crate) fn balanced(line: &str) -> bool {
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

/// Evaluates `expression` and renders its value the way `console` would
/// render a single argument.
pub(crate) fn rendered(expression: &str) -> String {
    with_realm(|ctx| {
        pin(&ctx).unwrap();
        let value: Value<'_> = ctx.eval(expression).unwrap();
        let out = line(&[value], false);
        assert!(!ctx.has_exception(), "a read left an exception pending");
        out
    })
}

/// Renders `expression` with a `calls` counter in scope, and returns the
/// text and the counter.
pub(crate) fn counted(expression: &str) -> (String, i32) {
    with_realm(|ctx| {
        pin(&ctx).unwrap();
        ctx.eval::<(), _>("globalThis.calls = 0;").unwrap();
        let value: Value<'_> = ctx.eval(expression).unwrap();
        let text = line(&[value], false);
        assert!(!ctx.has_exception());
        (text, ctx.eval("calls").unwrap())
    })
}

/// As [`rendered`], with colors on.
pub(crate) fn colored(expression: &str) -> String {
    with_realm(|ctx| {
        pin(&ctx).unwrap();
        let value: Value<'_> = ctx.eval(expression).unwrap();
        let out = line(&[value], true);
        assert!(!ctx.has_exception(), "a read left an exception pending");
        out
    })
}

pub(crate) fn yellow(text: &str) -> String {
    format!("\x1b[33m{text}\x1b[39m")
}

pub(crate) fn green(text: &str) -> String {
    format!("\x1b[32m{text}\x1b[39m")
}

pub(crate) fn cyan(text: &str) -> String {
    format!("\x1b[36m{text}\x1b[39m")
}

pub(crate) fn grey(text: &str) -> String {
    format!("\x1b[90m{text}\x1b[39m")
}

/// Renders `expression` through [`quoted_escaped`].
pub(crate) fn escaped(expression: &str, color: bool) -> String {
    with_realm(|ctx| {
        pin(&ctx).unwrap();
        let value: Value<'_> = ctx.eval(expression).unwrap();
        let out = quoted_escaped(&value, color);
        assert!(!ctx.has_exception(), "a read left an exception pending");
        out
    })
}

pub(crate) fn b64(text: &str) -> String {
    STANDARD.encode(text)
}

pub(crate) fn map_json(version: u32, mappings: &str, sources: &str, root: Option<&str>) -> String {
    let root = root.map_or(String::new(), |r| format!(r#","sourceRoot":"{r}""#));
    format!(r#"{{"version":{version},"mappings":"{mappings}","sources":{sources}{root}}}"#)
}

pub(crate) fn inline(json: &str) -> String {
    format!(
        "//# sourceMappingURL=data:application/json;base64,{}",
        b64(json)
    )
}

/// Registers `files` (name, generated source) and remaps `stack`.
pub(crate) fn remap(files: &[(&str, &str)], stack: &str) -> String {
    with_realm(|ctx| {
        for (name, source) in files {
            register(&ctx, name, source);
        }
        remap_stack(&ctx, stack)
    })
}

pub(crate) fn one(source: &str, stack: &str) -> String {
    remap(&[("a.js", source)], stack)
}
