<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-gpui

The [`gpui`](https://www.gpui.rs/) element and event layer inca renders
through. It renders the retained tree of `inca-node` into real `gpui` elements,
and needs nothing from a JS engine.

Input dispatch (which callback fires for which `(node, event)`) goes
through this crate's `EventSink` trait, which the caller implements.

The crate also holds the logic every host shares: the wheel wiring and the scrollbars of scroll
containers and the scroll-into-view box of a laid-out node (`rect_of`).
