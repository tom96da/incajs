<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# incajs

**Incarnative.js** — a GPU-native, Webview-free desktop application
framework. No Chromium, no DOM: UI renders directly on the GPU via
[GPUI](https://www.gpui.rs/), driven by an embedded QuickJS runtime
([`rquickjs`](https://github.com/DelSkayn/rquickjs)) with sub-second
startup.

## How it works

- `incajs` is a thin, typed wrapper around the native host bridge
- Vue 3 support is a custom renderer mapping Vue's `createRenderer`
  lifecycle onto those calls
- `@vue/runtime-core` is an optional peer dependency, needed only when
  you use it

Part of [tom96da/incajs](https://github.com/tom96da/incajs).
