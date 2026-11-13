<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Architecture

The concepts of Incarnative.js: its layers, how they connect and the development window.
[FFI.md](./FFI.md) holds the contract of the JS↔Rust boundary and [PROTOCOL.md](./PROTOCOL.md) the
dev protocol, HMR delivery included.

## Tech stack

| Layer | Technology | Primary role |
| --- | --- | --- |
| **Native core** | Rust + [`gpui`](https://www.gpui.rs/) (wgpu) | Window management, event loop, retained virtual tree, direct GPU rendering. |
| **JS engine** | QuickJS via [`rquickjs`](https://github.com/DelSkayn/rquickjs) | Embedded, lightweight JS runtime executing UI logic and reactivity. |
| **Core JS package** | `incajs` | Typed wrapper around the host bridge (`__inca_native__`), shared by every frontend adapter. |
| **Frontend framework** | `incajs/vue`, a subpath of the `incajs` package | Custom renderer mapping Vue's virtual component trees to `incajs` calls. |
| **Bundler & dev tooling** | Vite, in library/build mode | Compiles `.vue` through `@vitejs/plugin-vue`. |
| **Dev CLI** | `@incajs/cli` (Node) | Runs the `inca` commands and is the parent process during development. |
| **Bundler adapter** | `@incajs/cli`'s `vite` (Node) | Runs Vite and announces each rebuild. |
| **Host client** | `@incajs/cli`'s `dev-client` (Node) | Launches the host as a child and carries messages over its stdio. |
| **Host bridge** | In-process Rust functions bound into the QuickJS context via `rquickjs` | Carries mutation operations (`createNode`, `setAttribute`, `appendChild`, ...) from JS to the Rust host. No C ABI or IPC is involved, since everything runs in one process. |

## System architecture

```
┌──────────────────────────────────────────────────────────────────┐
│        [ @incajs/cli — Node process (dev only, parent) ]         │
│  [ .vue / .tsx ] ──▶ [ vite ──▶ Vite (watch) ]                   │
│                             │                                    │
│                       [ dev-client ]                             │
│                             │ fetchModule / HMR payloads         │
│                             │ (newline-delimited JSON over the   │
│                             │  child's stdio)                    │
└─────────────────────────────┼────────────────────────────────────┘
                              ▼  (dev-client spawns the host as a child)
┌──────────────────────────────────────────────────────────────────┐
│        [ Incarnative.js Native Runtime — host process ]          │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │ JS Runtime (QuickJS via `rquickjs`)                        │  │
│  │   - Vue 3 application (React: future)                      │  │
│  │   - Custom renderer (`createRenderer` / `react-reconciler`)│  │
│  │   - `incajs` core (`__inca_native__` wrapper, elements)    │  │
│  │   - Vite `ModuleRunner` + custom Transport                 │  │
│  │     (dev only; transformed modules run here, not in Node)  │  │
│  └──────────────────────────┬─────────────────────────────────┘  │
│                             │ Host bridge call                   │
│  ┌──────────────────────────▼─────────────────────────────────┐  │
│  │ Rust Host Core (GPUI engine)                               │  │
│  │   - Retained virtual tree (state & layout)                 │  │
│  │   - Host bridge dispatcher (`createNode`, `setAttribute`)  │  │
│  │   - GPUI element builder ──▶ [ Native GPU (wgpu/Metal) ]   │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

## Development window

The development window draws the application you wrote. `inca dev` opens it
with the prebuilt host.

- **Automatic reload**: the window reloads the application by itself each
  time a source file changes.
- **Output**: what the application prints reaches the terminal that ran
  `inca dev`.

With `inca dev --experimental-hmr`, an edit of a `.vue` file updates the running window in place.
A template-only edit and an edit of a style block keep the state of the components. A script edit
remounts the component. HMR runs on Vite's Runtime API and reuses Vite's module graph invalidation
and its accept and dispose boundaries. [PROTOCOL.md](./PROTOCOL.md#hmr-delivery) describes how the
updates reach the host.
