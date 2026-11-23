// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the unit tests of several modules.

#![allow(clippy::unwrap_used)]

mod fake_script;
pub(crate) mod focus;
pub(crate) mod input;
pub(crate) mod keys;
pub(crate) mod pointer;
pub(crate) mod scene;

pub(crate) use fake_script::*;
pub(crate) use focus::*;
pub(crate) use scene::*;
