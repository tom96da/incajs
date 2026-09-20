# Incarnative.js

[![npm version](https://img.shields.io/npm/v/incajs)](https://npmjs.com/package/incajs)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#scroll-license)
[![CI](https://github.com/tom96da/incajs/actions/workflows/ci.yml/badge.svg)](https://github.com/tom96da/incajs/actions/workflows/ci.yml)

**Incarnative.js** (`inca`) is an ultra-lightweight, Webview-free desktop application framework powered by **GPUI**, **QuickJS**, and custom renderers.

## :sparkles: Features

- :leaves: **Webview-Free Architecture**: Bypasses heavy Chromium/DOM overhead entirely.
- :zap: **Direct GPU Rendering**: Powered by Rust & GPUI for smooth, native-speed UI rendering.
- :rocket: **Micro-sized Runtime**: Employs QuickJS for sub-second startup times and minimal memory footprint.
- :art: **Frontend Agnostic**: Designed with first-class Vue 3 support, expanding to React and beyond.

## :wrench: Tech Stack

- :gear: **Engine / Core**: Rust (`gpui`)
- :yellow_heart: **JS Runtime**: QuickJS (`rquickjs`)
- :desktop_computer: **Frontend**: Vue 3 / Custom Renderer
- :package: **Bundler**: Vite, in library/build mode (not a browser dev server) — see [ARCHITECTURE.md](./specs/ARCHITECTURE.md)

## :rocket: Getting started

```sh [npm]
$ npm install incajs
$ npm install -D @incajs/cli
```

Drop a `src/App.vue`. No entry point or bootstrapping code needed:
`inca` mounts it as the app itself.

```vue
<script setup>
import { ref } from "@vue/runtime-core";

const clicks = ref(0);
</script>

<template>
  <div :style="{ text_color: 0xffffff }" @click="clicks++">
    Clicked {{ clicks }} time(s)
  </div>
</template>
```

Run `inca dev` for a live-reloading window. Run `inca build` for a
one-off bundle, or `inca package` for a distributable app.

## Host binary

`inca-host`'s prebuilt binary is pulled in automatically as a
platform-specific optional dependency. Neither Cargo nor Rust are
needed to run an app.

| Platform      | Status           |
| ------------- | ---------------- |
| Linux x64     | Limited support* |
| Linux arm64   | Limited support* |
| macOS arm64   | Supported        |
| macOS x64     | Not planned      |
| Windows x64   | Planned          |
| Windows arm64 | Planned          |

\* Packaged apps still need [these libraries](#linux-runtime-requirements)
on the machine that runs them — `inca package` doesn't bundle them yet.

## Linux runtime requirements

The host binary is built against glibc 2.35, so it needs Ubuntu 22.04,
Debian 12, or anything newer.

On Debian/Ubuntu, it also needs:

- `libxcb1`
- `libfontconfig1`
- `libfreetype6`
- `libxkbcommon0`
- `libxkbcommon-x11-0`
- `libvulkan1`
- a Vulkan driver package, such as `mesa-vulkan-drivers`

Other distributions use the same libraries under different package
names. They're present by default on most desktops, but a minimal or
headless install may need them added explicitly.

Some of these are already unnecessary. The binary bundles FreeType, and
`libfontconfig1` is linked on arm64 only. The list will be narrowed once
it is measured on a minimal install.

The Vulkan loader and driver are dlopen'd at startup rather than
linked, so `ldd` won't show them. Rendering can't start without them.

## :scroll: License

Dual-licensed under [MIT](./LICENSE-MIT) or [Apache 2.0](./LICENSE-APACHE).
