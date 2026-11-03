<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-bridge

Binds a `QuickJS` realm (`inca-jsenv`'s `Engine`) to `inca`'s retained
virtual tree (`inca-gpui`'s `VirtualTree`):

- `install` installs `globalThis.__inca_native__`: the small set of native
  functions JS calls to read the root handle, mutate the retained virtual
  tree, register input-event callbacks, dispatch an event with
  `dispatchEvent` or `click`, and free what it no longer needs. They also
  read the attribute, focus, layout and scroll offset of a node, and scroll a
  node or bring it into view. `Host::reveal_focus` carries out the reveal.
  `holdNode` keeps a parentless node and its subtree alive through the
  orphan sweep, and `releaseNode` ends the hold. `addEventListener`
  keeps the function it gets in the callback table of the engine. A script
  can neither replace nor delete `__inca_native__`.
- `EventDispatcher` carries a native input event the other way: from `gpui`
  into whichever JS callbacks are registered for it, read from the callback
  table of the engine, with one event object passed along the
  path of its target. It reads the objects that stand for the nodes of that
  path from `globalThis.__inca_targets__`. When a frame's orphan sweep frees
  nodes, it calls `globalThis.__inca_released__` with their ids and the ids
  of their callbacks. It implements `inca-gpui`'s `EventSink` trait so the
  render layer never has to depend on `QuickJS` itself. A window calls
  `EventDispatcher::begin_frame` first in its render and
  `EventDispatcher::window_activated` when it gains or loses focus.
- `install` also defines the timer functions and `requestAnimationFrame` of
  the global object, over the timers and frame callbacks of the `Host`.
  `EventDispatcher::start_timers` runs the due timers in a foreground task per
  window, and the frame callbacks in the next frame of the window while one is
  pending.
- `focus` re-exports `inca-gpui`'s `focus` module: `FocusRegistry` keeps a
  `gpui::FocusHandle` per node and reports the focus and blur transitions
  that `EventDispatcher::begin_frame` dispatches.

This is where `gpui` and `QuickJS` actually meet. `inca-gpui` and
`inca-jsenv` know nothing about each other.
