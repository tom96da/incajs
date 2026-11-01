// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Safe wrappers over `QuickJS` calls.
//!
//! The quickjs module and its children are the only code of the crate that
//! import `rquickjs::qjs` or write `unsafe` code. The wrappers take and return
//! owned `rquickjs` types and report failure as [`rquickjs::Error::Exception`]
//! with the exception left pending. They run the script code that the underlying `QuickJS` call
//! runs and no more. They use no type of this crate.
//!
//! A panic that a Rust callback stored during a call stays with the runtime
//! until `rquickjs` next resumes it.

// The workspace denies `unsafe_code`, and this module is its one exception.
#![allow(unsafe_code)]

mod value;

pub use value::FunctionAccessor;
pub(crate) use value::{ObjectExt, ValueExt};
