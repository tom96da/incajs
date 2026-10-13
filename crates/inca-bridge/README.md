<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-bridge

Binds a `QuickJS` realm (`inca-jsenv`'s `Engine`) to `inca`'s retained
virtual tree (`inca-gpui`'s `VirtualTree`):

- `bindings` installs `globalThis.__inca_native__` — the small set of native
  functions JS calls to read the root handle, mutate the retained virtual
  tree, register input-event callbacks, and free what it no longer needs.
  They also read the attribute, focus, layout and scroll offset of a node,
  and scroll a node or bring it into view. `Host::reveal_focus` carries out the reveal.
- `dispatch` carries a native input event the other way: from `gpui` into
  whichever JS callbacks are registered for it, via
  `globalThis.__inca_callbacks__`, with one event object passed along the
  path of its target. It implements `inca-gpui`'s `EventSink` trait so the
  render layer never has to depend on `QuickJS` itself. A window calls
  `EventDispatcher::begin_frame` first in its render and
  `EventDispatcher::window_activated` when it gains or loses focus.
- `focus` re-exports `inca-gpui`'s `focus` module: `FocusRegistry` keeps a
  `gpui::FocusHandle` per node and dispatches `"focus"`/`"blur"` when the
  focused node changes.

This is where `gpui` and `QuickJS` actually meet — `inca-gpui` and
`inca-jsenv` know nothing about each other.
