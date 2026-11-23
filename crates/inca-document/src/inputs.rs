// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The operations of the document on the text states of its `input` nodes.

use inca_node::NodeId;

use crate::{DocumentState, InputState};

impl DocumentState {
    /// The text state of the `input` node `node`, `None` before its first use.
    #[must_use]
    pub fn input(&self, node: NodeId) -> Option<&InputState> {
        self.inputs.get(&node)
    }

    /// Sets the typed text of the `input` node `node` to `value`, creating its text state on first
    /// use.
    pub fn set_input_value(&mut self, node: NodeId, value: &str) {
        self.edit_input(node, |input| input.set_value(value));
    }

    /// Runs `edit` on the text state of the `input` node `node`, created on first use, and returns
    /// what `edit` returns.
    pub fn edit_input<R>(&mut self, node: NodeId, edit: impl FnOnce(&mut InputState) -> R) -> R {
        edit(self.inputs.entry(node).or_default())
    }
}
