// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The focus plan: what the window does to its handles and to its focus for one frame or one Tab
//! press.

use inca_node::NodeId;

use super::FocusTransition;

/// What the window does to the platform focus this frame, in order: it creates the handles of
/// `gained`, runs `steps`, drops the handles of `lost` and parks when `park` is set.
#[must_use]
#[derive(Debug, Default)]
pub struct FocusPlan {
    /// The nodes that take focus as of this check. Each gets a handle when it has none, before
    /// the steps run.
    pub gained: Vec<NodeId>,
    /// The moves of the platform focus.
    pub steps: Vec<FocusStep>,
    /// The nodes that no longer take focus. Their handles drop after the steps.
    pub lost: Vec<NodeId>,
    /// Whether the window gives its focus to the handle that maps to no node when it rests on no
    /// node, so key events reach the root container.
    pub park: bool,
    /// The transitions to report, in order.
    pub transitions: Vec<FocusTransition>,
    /// What happens to the reveals a focus asked for. [`crate::RevealQueue::follow_plan`] applies
    /// it.
    pub(crate) reveal: FocusReveal,
}

/// One move of the platform focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusStep {
    /// Focus the handle of the node.
    Focus(NodeId),
    /// Release the window's focus.
    Blur,
}

/// What a plan does to the reveals a focus asked for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FocusReveal {
    /// They stay.
    #[default]
    Keep,
    /// They drop, and the node, when set, is revealed. The last focus decides what scrolls.
    Replace(Option<NodeId>),
}
