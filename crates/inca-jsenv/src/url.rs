// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `URL` and `URLSearchParams` classes.
//!
//! A URL and its `searchParams` object share one `State`: the parsed URL and the list of pairs of
//! its query. Reads use the list. The `search` and `href` setters rebuild the list, and every
//! mutation of the list writes the serialization back to the URL. The prototypes sit in the vault,
//! so a script that replaces a property of a prototype or a constructor changes nothing here.
//! This module installs both classes and holds the state and the shared conversions. The
//! `url_class` module holds `URL`, the `params` module holds `URLSearchParams`, the
//! `params_init` module converts the argument of its constructor and the `iterator` module holds
//! its iterator.

// A derive on a struct with a lifetime generates items that an `allow` on the struct does not
// cover, so the struct sits in a module of its own.
#[allow(clippy::disallowed_macros, clippy::disallowed_types)]
mod instance;
mod iterator;
mod params;
mod params_init;
mod state;
mod url_class;

pub(crate) use state::{Printed, install, printed};
