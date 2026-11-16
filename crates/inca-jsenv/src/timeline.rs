// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The performance timeline. The `entries` module holds the entry classes, the timeline store, the
//! reads and clears and the install step. The `record` module holds the arguments of `mark()` and
//! `new PerformanceMark()` and the entries they make, and the `measure` module those of
//! `measure()`.

mod entries;
mod measure;
mod record;

pub(crate) use entries::install;
