// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use crate::tests::{strip, with_realm};
use rquickjs::{Context, Runtime};

mod arrays;
mod collections;
mod color;
mod objects;
mod proxy;

/// Evaluates `expression` and renders its value the way `console` would
/// render a single argument.
fn rendered(expression: &str) -> String {
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
fn counted(expression: &str) -> (String, i32) {
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
fn colored(expression: &str) -> String {
    with_realm(|ctx| {
        pin(&ctx).unwrap();
        let value: Value<'_> = ctx.eval(expression).unwrap();
        let out = line(&[value], true);
        assert!(!ctx.has_exception(), "a read left an exception pending");
        out
    })
}

fn yellow(text: &str) -> String {
    format!("\x1b[33m{text}\x1b[39m")
}

fn green(text: &str) -> String {
    format!("\x1b[32m{text}\x1b[39m")
}

fn cyan(text: &str) -> String {
    format!("\x1b[36m{text}\x1b[39m")
}

fn grey(text: &str) -> String {
    format!("\x1b[90m{text}\x1b[39m")
}

/// Renders `expression` through [`quoted_escaped`].
fn escaped(expression: &str, color: bool) -> String {
    with_realm(|ctx| {
        pin(&ctx).unwrap();
        let value: Value<'_> = ctx.eval(expression).unwrap();
        let out = quoted_escaped(&value, color);
        assert!(!ctx.has_exception(), "a read left an exception pending");
        out
    })
}
