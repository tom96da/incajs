<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# incajs

**Incarnative.js** — a GPU-native, Webview-free desktop application
framework powered by [GPUI](https://www.gpui.rs/) and QuickJS.

`incajs` provides the framework-agnostic JS API that talks directly to the
native host bridge (`globalThis.__gpjsui_native__`), plus first-class Vue 3
support via the `incajs/vue` subpath — a custom renderer mapping Vue's
`createRenderer` lifecycle methods onto this package's own typed wrapper
functions. `incajs/vue`'s `@vue/runtime-core` peer dependency is optional:
only import `incajs/vue` if you're building with Vue.

The typed wrapper functions over the native bridge, the
`__gpjsui_callbacks__` event-listener registry, and the Vue 3 renderer
(node lifecycle, text/comment nodes, style/attribute/event prop patching)
are implemented and tested.

Part of [tom96da/incajs](https://github.com/tom96da/incajs).
