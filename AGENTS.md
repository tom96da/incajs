<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Incarnative.js

**Incarnative.js** (`inca`, npm: `incajs`) is an ultra-lightweight, Webview-free desktop application framework powered by **GPUI**, **QuickJS**, and custom renderers. Write Vue 3 components, get native windows drawn directly on the GPU. No Chromium, no DOM.

## Why

- **Webview-free**: no browser engine to ship or boot.
- **Direct GPU rendering**: Rust and GPUI draw the UI natively.
- **Micro-sized runtime**: QuickJS gives sub-second startup and a small memory footprint.
- **Standard Vue 3**: author ordinary `.vue` single-file components.

## How it works

```
.vue source ─ Vite (build mode) ─▶ bundle
bundle ─ QuickJS ─ custom Vue renderer ─ FFI ─▶ Rust retained tree
Rust retained tree ─ layout + paint ─▶ GPUI ─▶ GPU
```

JS never runs per frame. It runs only when reactive state changes and pushes
the mutations to Rust, which owns the tree and does layout and drawing
natively. See [ARCHITECTURE.md](./handbook/ARCHITECTURE.md) for the details.

## Status

Early stage. Vue 3 is the only supported frontend. The CLI (`inca dev`,
`inca build`, `inca package`), HMR, pointer, keyboard and scroll input work.
Text editing and IME are not built yet.

## Links

- Docs: [tom96da.github.io/incajs](https://tom96da.github.io/incajs/)
- npm: [`incajs`](https://npmjs.com/package/incajs), [`@incajs/cli`](https://npmjs.com/package/@incajs/cli)
- [README.md](./README.md), [CHANGELOG.md](./CHANGELOG.md)

Developing in this repository? Read [CONTRIBUTING.md](./CONTRIBUTING.md).
