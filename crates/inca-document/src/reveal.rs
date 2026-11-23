// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reveals that wait for a frame: a focus that scrolls its node into view, and a
//! `scrollIntoView` call whose node has no box yet.

use inca_node::{NodeId, VirtualTree};

use crate::FocusPlan;
use crate::focus::FocusReveal;
use crate::geometry::Align;

/// What asked for a reveal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealOrigin {
    /// A focus request that scrolls its node into view.
    Focus,
    /// A `scrollIntoView` call.
    Script,
}

/// A node to scroll into view with the block and inline alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reveal {
    pub node: NodeId,
    pub block: Align,
    pub inline: Align,
    pub origin: RevealOrigin,
}

impl Reveal {
    /// The reveal a focus request asks for.
    #[must_use]
    pub fn focus(node: NodeId) -> Self {
        Self {
            node,
            block: Align::Reveal,
            inline: Align::CenterOutside,
            origin: RevealOrigin::Focus,
        }
    }
}

/// The reveals of one frame, as [`RevealQueue::take`] hands them over.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reveals {
    /// The reveals whose node lacked a box in the previous frame. They run once more.
    pub retries: Vec<Reveal>,
    /// The reveals queued since the previous frame.
    pub fresh: Vec<Reveal>,
}

/// The reveals that wait for the next frame, oldest first.
#[derive(Debug, Default)]
pub struct RevealQueue {
    fresh: Vec<Reveal>,
    retry: Vec<Reveal>,
}

impl RevealQueue {
    /// Queues `reveal` for the next frame.
    pub fn queue(&mut self, reveal: Reveal) {
        self.fresh.push(reveal);
    }

    /// Keeps `reveal`, whose node had no box, for one more frame.
    pub fn retry(&mut self, reveal: Reveal) {
        self.retry.push(reveal);
    }

    /// Hands over the reveals of this frame.
    pub fn take(&mut self) -> Reveals {
        Reveals {
            retries: std::mem::take(&mut self.retry),
            fresh: std::mem::take(&mut self.fresh),
        }
    }

    /// Drops every reveal a focus asked for. The last focus decides what scrolls.
    pub fn clear_focus(&mut self) {
        self.retain(|reveal| reveal.origin != RevealOrigin::Focus);
    }

    /// Applies what `plan` does to the reveals a focus asked for.
    pub fn follow_plan(&mut self, plan: &FocusPlan) {
        if let FocusReveal::Replace(node) = plan.reveal {
            self.clear_focus();
            if let Some(node) = node {
                self.queue(Reveal::focus(node));
            }
        }
    }

    /// Drops the reveals a focus asked for of `node_id` and of the nodes below it.
    pub fn drop_focus_in(&mut self, tree: &VirtualTree, node_id: NodeId) {
        self.retain(|reveal| {
            reveal.origin != RevealOrigin::Focus
                || !tree
                    .ancestors(reveal.node)
                    .any(|ancestor| ancestor == node_id)
        });
    }

    /// Drops every reveal of the freed node `node_id`.
    pub fn forget(&mut self, node_id: NodeId) {
        self.retain(|reveal| reveal.node != node_id);
    }

    fn retain(&mut self, keep: impl Fn(&Reveal) -> bool) {
        self.fresh.retain(&keep);
        self.retry.retain(&keep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rooted_tree;

    fn script(node: NodeId) -> Reveal {
        Reveal {
            node,
            block: Align::Start,
            inline: Align::Start,
            origin: RevealOrigin::Script,
        }
    }

    #[test]
    fn a_reveal_retry_comes_back_as_a_retry() {
        let mut queue = RevealQueue::default();
        queue.retry(Reveal::focus(4));

        assert_eq!(
            queue.take(),
            Reveals {
                retries: vec![Reveal::focus(4)],
                fresh: vec![]
            }
        );
    }

    #[test]
    fn a_queued_reveal_is_handed_over_once() {
        let mut queue = RevealQueue::default();
        queue.queue(script(4));

        assert_eq!(queue.take().fresh, [script(4)]);
        assert_eq!(queue.take(), Reveals::default());
    }

    #[test]
    fn clearing_the_focus_reveals_keeps_the_script_ones() {
        let mut queue = RevealQueue::default();
        queue.queue(Reveal::focus(1));
        queue.queue(script(2));
        queue.retry(Reveal::focus(3));
        queue.retry(script(4));

        queue.clear_focus();

        assert_eq!(
            queue.take(),
            Reveals {
                retries: vec![script(4)],
                fresh: vec![script(2)]
            }
        );
    }

    #[test]
    fn a_drop_takes_only_the_focus_reveals_inside_the_subtree() {
        let (tree, ids) = rooted_tree(2);
        let mut queue = RevealQueue::default();
        queue.queue(Reveal::focus(ids[0]));
        queue.queue(script(ids[0]));
        queue.retry(Reveal::focus(ids[1]));

        queue.drop_focus_in(&tree, ids[0]);

        assert_eq!(
            queue.take(),
            Reveals {
                retries: vec![Reveal::focus(ids[1])],
                fresh: vec![script(ids[0])]
            }
        );
    }

    #[test]
    fn forget_drops_every_reveal_of_the_node() {
        let mut queue = RevealQueue::default();
        queue.queue(Reveal::focus(1));
        queue.retry(script(1));
        queue.queue(script(2));

        queue.forget(1);

        assert_eq!(queue.take().fresh, [script(2)]);
    }
}
