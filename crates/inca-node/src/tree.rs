// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Retained virtual tree.
//!
//! `VirtualTree` owns every `VirtualNode` and hands out a [`NodeId`] for each.
//!
//! Two invariants the render path relies on: a node has at most one parent
//! (attaching it somewhere new detaches it from where it was), and an
//! attachment that would make a node its own ancestor is refused, so a walk
//! down the tree always terminates. A fragment never has a parent.
//!
//! A template owns a fragment as its content. The content has no parent and is never connected.
//!
//! A document can be declared with [`VirtualTree::set_document`]. It is the top of the tree and
//! [`VirtualTree::is_connected`] reads the parent chain up to it. A root declared with
//! [`VirtualTree::set_root`] is never freed by the sweep. Without a
//! document, `is_connected` measures against the root.
//!
//! [`VirtualTree::remove_child`] detaches a node and keeps it with its
//! children.
//! [`VirtualTree::destroy_node`] frees a subtree on request.
//!
//! An orphan is a node with no parent that is neither the root, the document nor held with
//! [`VirtualTree::hold`]. [`VirtualTree::sweep`] frees every orphan with its
//! subtree. A node that is created, detached or released is an orphan until
//! it is inserted, held or destroyed.

mod class_list;
mod clone;
mod collection;
mod insert_nodes;
mod insertion;
mod lifetime;
mod links;
mod name;
mod node;
mod props;
mod relations;
mod state;
mod store;
mod style_key;
mod template;
mod text;

pub use class_list::ClassListError;
pub use insert_nodes::{InsertArg, Inserted, Insertion, Placement};
pub use lifetime::Removal;
pub use props::valid_attribute_name;
pub use relations::Link;
pub use style_key::{
    AlignSpec, DisplaySpec, FlexDirectionSpec, OverflowSpec, STYLE_KEYS, StyleKey, ValueKind,
};
pub use style_key::{css_name, style_key};

pub(crate) use name::COMMENT_TAG;
#[cfg(test)]
pub(crate) use name::DOCUMENT_TAG;
pub use name::{FRAGMENT_TAG, NodeKind, TEXT_TAG};
pub use node::{
    AttributeValue, NodeId, StyleMap, VirtualNode, is_disabled_control, shortest_number_text,
};
pub use store::{TreeError, VirtualTree};
