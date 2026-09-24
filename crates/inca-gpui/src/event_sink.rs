// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::Window;

use crate::tree::NodeId;

/// Defines [`EventKind`] and its `ALL` slice from one variant/name list, so
/// a variant can't be added to the enum without also landing in `ALL`.
macro_rules! event_kinds {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        /// A native event kind `inca-gpui` can wire a node for.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum EventKind {
            $($variant),+
        }

        impl EventKind {
            /// Every kind, in no particular order.
            pub const ALL: &[Self] = &[$(Self::$variant),+];

            /// The name `addEventListener`/`EventSink::dispatch` use for this kind.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }
        }
    };
}

event_kinds! {
    Click => "click",
}

impl EventKind {
    /// The bit [`EventMask`] uses for this kind.
    #[must_use]
    pub const fn mask(self) -> EventMask {
        match self {
            Self::Click => EventMask::CLICK,
        }
    }
}

/// Which of [`EventKind::ALL`] a node is wired for, as one bit per kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventMask(u16);

impl EventMask {
    /// Wired for nothing.
    pub const NONE: Self = Self(0);
    /// Wired for [`EventKind::Click`].
    pub const CLICK: Self = Self(1 << 0);

    /// Whether every bit set in `other` is also set in `self`.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The bit `name` occupies, or [`EventMask::NONE`] if `name` names no
    /// [`EventKind`].
    #[must_use]
    pub fn for_name(name: &str) -> Self {
        EventKind::ALL
            .iter()
            .find(|kind| kind.name() == name)
            .map_or(Self::NONE, |kind| kind.mask())
    }
}

impl std::ops::BitOr for EventMask {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// What a dispatch carries beyond the node it fired on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPayload {
    /// No data beyond the node and the event's name.
    None,
}

/// What [`crate::element`] needs from something that can dispatch a native
/// event into JS — `inca-bridge`'s `EventDispatcher` implements this, kept
/// as a trait here rather than a direct dependency so this crate never has
/// to depend on `QuickJS`.
pub trait EventSink {
    /// Every kind registered on `node_id`. The render path asks before
    /// wiring an element for input.
    fn listens(&self, node_id: NodeId) -> EventMask;

    /// Calls whatever is registered for `(node_id, event)`, passing `payload`.
    fn dispatch(&self, node_id: NodeId, event: &str, payload: &EventPayload, window: &mut Window);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_contains_only_none() {
        assert!(EventMask::NONE.contains(EventMask::NONE));
        assert!(!EventMask::NONE.contains(EventMask::CLICK));
    }

    #[test]
    fn a_mask_contains_itself_and_none() {
        assert!(EventMask::CLICK.contains(EventMask::CLICK));
        assert!(EventMask::CLICK.contains(EventMask::NONE));
    }

    #[test]
    fn union_contains_both_operands() {
        let union = EventMask::CLICK | EventMask::NONE;
        assert!(union.contains(EventMask::CLICK));
    }

    /// Every kind carries its own bit; no two share one.
    #[test]
    fn every_kind_has_its_own_bit() {
        let mut seen = EventMask::NONE;
        for kind in EventKind::ALL {
            let bit = kind.mask();
            assert_ne!(bit, EventMask::NONE, "{kind:?} has no bit");
            assert!(
                !seen.contains(bit),
                "{kind:?}'s bit collides with an earlier kind's"
            );
            seen = seen | bit;
        }
    }

    #[test]
    fn for_name_matches_every_kind() {
        for kind in EventKind::ALL {
            assert_eq!(EventMask::for_name(kind.name()), kind.mask());
        }
        assert_eq!(EventMask::for_name("not-a-real-event"), EventMask::NONE);
    }

    #[test]
    fn no_two_kinds_share_a_name() {
        let mut names = std::collections::HashSet::new();
        for kind in EventKind::ALL {
            assert!(
                names.insert(kind.name()),
                "{kind:?} repeats an earlier name"
            );
        }
    }
}
