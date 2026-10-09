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
- `dispatch` carries a native input event the other way: from `gpui` into
  whichever JS callbacks are registered for it, via
  `globalThis.__inca_callbacks__`, implementing `inca-gpui`'s `EventSink`
  trait so the render layer never has to depend on `QuickJS` itself.
- `dev` installs `globalThis.__inca_dev__` — the relay a bundler integration
  uses to exchange messages with the host, and `setLoadFailed`, which the app
  calls to report that its load failed or recovered.
- `focus` holds `FocusRegistry`, which keeps a `gpui::FocusHandle` per node
  and dispatches `"focus"`/`"blur"` when the focused node changes.

This is where `gpui` and `QuickJS` actually meet — `inca-gpui` and
`inca-jsenv` know nothing about each other.
