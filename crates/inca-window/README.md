<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-window

The behaviour of a window: the input it receives turned into events and default actions, and the
state that lives with a [`gpui`](https://www.gpui.rs/) window.

`EventDispatcher` implements the `EventSink` of `inca-gpui`. It fills the payload fields that come
from the window, runs the default actions (clicks, hover, key defaults, text editing and
composition, scrollbars) and decides the order of a frame. It reaches the script only through the
`Script` trait, which the caller implements.

`WindowState` holds what lives with the window: the boxes of the last drawn frame, the scroll
handles, the held mouse buttons, the active pointer capture and the focus handles. It implements
the `View` of `inca-document`.
