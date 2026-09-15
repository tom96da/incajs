# Incarnative.js

**Incarnative.js** (`inca`) is an ultra-lightweight, Webview-free desktop application framework powered by **GPUI**, **QuickJS**, and custom renderers.

## :sparkles: Features

- :leaves: **Webview-Free Architecture**: Bypasses heavy Chromium/DOM overhead entirely.
- :zap: **Direct GPU Rendering**: Powered by Rust & GPUI for smooth 120fps UI performance.
- :rocket: **Micro-sized Runtime**: Employs QuickJS for sub-second startup times and minimal memory footprint.
- :art: **Frontend Agnostic**: Designed with first-class Vue 3 support, expanding to React and beyond.

## :wrench: Tech Stack

- :gear: **Engine / Core**: Rust (`gpui`)
- :yellow_heart: **JS Runtime**: QuickJS (`rquickjs`)
- :desktop_computer: **Frontend**: Vue 3 / Custom Renderer
- :package: **Bundler**: Vite, in library/build mode (not a browser dev server) — see [ARCHITECTURE.md](./specs/ARCHITECTURE.md)

## :scroll: License

Dual-licensed under [MIT](./LICENSE-MIT) or [Apache 2.0](./LICENSE-APACHE).
