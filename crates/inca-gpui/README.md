<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-gpui

The retained virtual tree and the [`gpui`](https://www.gpui.rs/) element/event
layer inca renders through. No QuickJS dependency of its own — the tree is a
plain data structure any caller can build and mutate, and rendering it into
real `gpui` elements needs nothing from a JS engine.

The tree is single-parent and freed explicitly: attaching a node detaches it
from where it was, and `destroyNode` is what releases one and everything
below it. Input dispatch (which callback fires for which `(node, event)`) is
behind this crate's `EventSink` trait rather than a concrete dependency —
`inca-bridge`'s `EventDispatcher` is what actually implements it, wiring
input back into JS.
