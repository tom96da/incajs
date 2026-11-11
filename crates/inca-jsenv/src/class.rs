// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The steps that define a name a Web standard defines, and the checks its native functions make
//! when a script calls them.
//!
//! The `define` module holds the definition steps: a class with its constructor, methods and
//! getters, a free function and a namespace object. The `guard` module holds the call-time checks:
//! `new` on a constructor, the receiver of a method, the argument count and the prototype of a new
//! instance. This module re-exports both.

mod define;
mod guard;

pub use crate::plain_class;
pub use define::{
    define_accessor, define_class, define_function, define_getter, define_global, define_method,
    define_namespace, define_this_accessor, define_this_getter, define_this_method,
    getter_function, iterator_prototype,
};
pub use guard::new_target_prototype;
pub(crate) use guard::{NEEDS_NEW, given, illegal_constructor};
pub use guard::{receiver, require};
