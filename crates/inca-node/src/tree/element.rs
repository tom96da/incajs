// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Creating an element of a namespace.

use html5ever::{Namespace, ns};

use super::{NodeId, TreeError, VirtualTree, is_template_tag};

impl VirtualTree {
    /// Allocates an element of `namespace` named `local_name` under `prefix`. An empty or absent
    /// namespace leaves the element without one. Only an element of the HTML namespace named
    /// `template` owns a content fragment.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::IdsExhausted`] as [`create_node`](Self::create_node) does.
    pub fn create_element_ns(
        &mut self,
        namespace: Option<&str>,
        prefix: Option<&str>,
        local_name: impl Into<String>,
    ) -> Result<NodeId, TreeError> {
        let namespace = namespace
            .filter(|text| !text.is_empty())
            .map(Namespace::from);
        self.create_element(namespace, prefix.map(str::to_owned), local_name.into())
    }

    pub(super) fn create_element(
        &mut self,
        namespace: Option<Namespace>,
        prefix: Option<String>,
        tag_name: String,
    ) -> Result<NodeId, TreeError> {
        if namespace == Some(ns!(html)) && is_template_tag(&tag_name) {
            return self.create_template(tag_name, prefix);
        }
        self.allocate(tag_name, namespace, prefix)
    }
}
