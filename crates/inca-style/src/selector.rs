// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! CSS selector matching over the retained tree, for `matches()`, `closest()` and the two query
//! methods.

mod element_kind;
mod matching;
mod parse;
mod query;
mod state;

use matching::Node;
pub use matching::{closest_selector, matches_selector};
use parse::{Atom, Impl, PseudoClass, PseudoElement, parse};
pub use parse::{SelectorError, check_selector};
pub(crate) use parse::{Selectors, StateFlags};
pub use query::{query_selector, query_selector_all};
