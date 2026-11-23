// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text, caret, selection and composition of an `input` node, in byte
//! offsets of its value on char boundaries, and the edits that change them.
//! The platform's text input speaks UTF-16 code units; the conversions are
//! here.

mod edit;
mod state;

pub use edit::{Edit, InputPayload, InputType};
pub use state::{InputState, Motion};
