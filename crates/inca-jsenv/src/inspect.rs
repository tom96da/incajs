// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders a JS value as the text `console` prints.
//!
//! Modelled on Node's `util.inspect`. `JSON.stringify` throws on a cycle and
//! drops functions, `undefined` and symbols, which are the values someone
//! reaches for `console` to look at.

mod array;
mod builtins;
mod collections;
mod object;
mod rect;
mod url;
mod value;

pub(crate) use builtins::pin;
pub(crate) use value::{
    detailed, escape_controls, line, quote, quoted, quoted_escaped, settled, unproxied,
};
