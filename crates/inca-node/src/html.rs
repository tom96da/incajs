// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Parsing HTML into the tree.
//!
//! [`VirtualTree::parse_html`] reads a markup fragment with the HTML fragment parsing algorithm
//! and builds the nodes directly in the tree. The result is a fragment that has no parent and is
//! not held, so the caller moves its children and destroys it in the same task.
//!
//! [`VirtualTree::serialize_html`] writes a subtree back as markup.
//!
//! An element keeps the namespace the parser reports. The names and attribute keys of the HTML
//! namespace are in lower case, and those of `svg` and `math` content keep the case the parser
//! delivers, such as `viewBox`. A `style` attribute goes to the hook of [`VirtualTree::parse_html_with`] when its element is
//! created. [`VirtualTree::parse_html`] stores it as written.

mod depth;
mod parse;
mod serialize;
mod sink;

use parse::StyleHook;
pub use parse::{ParsedHtml, ParsedNode};
pub use serialize::HtmlScope;
