<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-gpui

The retained virtual tree and the [`gpui`](https://www.gpui.rs/) element/event
layer inca renders through. It depends on `gpui` only. The tree is a
plain data structure any caller can build and mutate, and rendering it into
real `gpui` elements needs nothing from a JS engine.

The tree is single-parent: attaching a node detaches it from where it was.
A parentless node that is neither the root nor held is an orphan, and
`VirtualTree::sweep` frees every orphan with its subtree. The host sweeps at
the start of each frame. `hold` keeps a node out of the sweep and `release`
ends the hold. `destroyNode` frees one node and its subtree at once.

Input dispatch (which callback fires for which `(node, event)`) goes
through this crate's `EventSink` trait, which the caller implements.

The crate also holds the logic every host shares: the `focus` module
(per-node focus handles and the Tab order), the scroll-into-view geometry
(`element::scroll`), and the event table (`EVENTS`) with each event's
`bubbles`, `cancelable` and `composed` flags.
