// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Node` interface: the natives that allocate, link, free, mutate and clone nodes, the node
//! handle with the rules that check a receiver, and the members of `Node`, `CharacterData` and
//! `Text`.

mod character_data;
mod clone_node;
pub(crate) mod handle;
mod insert_nodes;
mod links;
mod mutation;
mod tree;

pub(crate) use handle::install_handle;
pub(crate) use insert_nodes::install_insert_nodes;
pub(crate) use links::install as install_members;
pub(crate) use links::install_node;
pub(crate) use mutation::{install_mutation, removed_subtrees};
pub(crate) use tree::install_tree;
