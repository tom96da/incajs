// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The performance timeline. The `entries` module holds the entry classes, the timeline store, the
//! reads and clears and the install step. The `record` module holds the arguments of `mark()`,
//! `new PerformanceMark()` and `measure()` and the entries they make.

mod entries;
mod record;

pub(crate) use entries::install;
