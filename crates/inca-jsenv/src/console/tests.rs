// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;

use rquickjs::{Context, Runtime};

use super::*;

mod color;
mod format;
mod methods;
mod table;

/// Evaluates `source` against a realm with `console` installed, and
/// returns every line it wrote.
fn lines_written_by(source: &str) -> Vec<(Level, String)> {
    written_with_color(source, false)
}

/// Runs `run` against a realm with `console` installed, and returns every
/// line it wrote.
fn written_by(color: bool, run: impl FnOnce(&Ctx<'_>)) -> Vec<(Level, String)> {
    let written = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&written);
    let output: Output = Rc::new(move |level, message| {
        sink.borrow_mut().push((level, message.to_owned()));
    });

    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    context.with(|ctx| {
        install(&ctx, &output, color).unwrap();
        run(&ctx);
        assert!(!ctx.has_exception(), "a call left an exception pending");
    });

    written.take()
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

/// `text` without the color sequences this crate paints.
fn without_known_codes(text: &str) -> String {
    let mut out = text.to_owned();
    for code in [31, 32, 33, 34, 35, 36, 1, 90, 39, 22] {
        out = out.replace(&format!("\x1b[{code}m"), "");
    }
    out
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

/// As [`lines_written_by`], with color switched on or off.
fn written_with_color(source: &str, color: bool) -> Vec<(Level, String)> {
    written_by(color, |ctx| ctx.eval::<(), _>(source).unwrap())
}

fn colored_texts(source: &str) -> Vec<String> {
    written_with_color(source, true)
        .into_iter()
        .map(|(_, t)| t)
        .collect()
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
