<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-bridge

Binds a `QuickJS` realm (`inca-jsenv`'s `Engine`) to `inca`'s retained
virtual tree (`inca-node`'s `VirtualTree`):

- `install` installs `globalThis.__inca_native__`: the small set of native
  functions JS calls to read the root handle, mutate the retained virtual
  tree, register input-event callbacks, dispatch an event with
  `dispatchEvent` or `click`, and free what it no longer needs. They also
  read the attribute, focus, layout and scroll offset of a node, and scroll a
  node or bring it into view.
  `holdNode` keeps a parentless node and its subtree alive through the
  orphan sweep, and `releaseNode` ends the hold. `addEventListener`
  keeps the function it gets in the callback table of the engine. A script
  can neither replace nor delete `__inca_native__`.
- `JsScript` runs the turns of the engine a window asks for: the listeners of an event, read from
  the callback table of the engine, with one event object passed along the path of its target. It
  reads the objects that stand for the nodes of that path from `globalThis.__inca_targets__`. When
  nodes are freed, and when a signal aborts or is collected, it calls
  `globalThis.__inca_released__` with their ids and the ids of their callbacks.
- `install` also defines `AbortController` and `AbortSignal`.
- `install` also defines the timer functions and `requestAnimationFrame` of
  the global object, over the timers and frame callbacks of the `Host`. `JsScript` runs the due
  timers and the pending frame callbacks.
- The `Host` reaches the window only through the `View` of `inca-document`.
