<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# incajs

**Incarnative.js** — a GPU-native, Webview-free desktop application
framework. No Chromium, no DOM: UI renders directly on the GPU via
[GPUI](https://www.gpui.rs/), driven by an embedded QuickJS runtime
([`rquickjs`](https://github.com/DelSkayn/rquickjs)).

## How it works

- The root `incajs` export is types only.
- `incajs/vue` is a custom renderer mapping Vue 3's `createRenderer`
  lifecycle onto the native host bridge. `createIncaApp(rootComponent,
  rootProps?)` creates a Vue app whose root mounts against the host's
  own root container instead of a DOM element; `app.mount()` with no
  argument targets it, and `app.unmount`, `app.use`, etc. behave exactly
  as `@vue/runtime-core` documents them.
- `@vue/runtime-core` is an optional peer dependency, needed only when
  you use `incajs/vue`.

Part of [tom96da/incajs](https://github.com/tom96da/incajs).
