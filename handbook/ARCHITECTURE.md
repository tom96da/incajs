<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Architecture

The layers of Incarnative.js and how they connect. [FFI.md](./FFI.md) covers
the JS↔Rust binding surface and [PROTOCOL.md](./PROTOCOL.md) the dev protocol.

## Tech stack

| Layer | Technology | Primary role |
| --- | --- | --- |
| **Native core** | Rust + [`gpui`](https://www.gpui.rs/) (wgpu) | Window management, event loop, retained virtual tree, direct GPU rendering. |
| **JS engine** | QuickJS via [`rquickjs`](https://github.com/DelSkayn/rquickjs) | Embedded, lightweight JS runtime executing UI logic and reactivity. |
| **Core JS package** | `incajs` | Typed wrapper around the host bridge (`__inca_native__`), shared by every frontend adapter. |
| **Frontend framework** | `incajs/vue`, a subpath of the `incajs` package | Custom renderer mapping Vue's virtual component trees to `incajs` calls. |
| **Bundler & dev tooling** | Vite, in library/build mode | Compiles `.vue` through `@vitejs/plugin-vue`. HMR is delivered through Vite's Runtime API (see [HMR delivery](#hmr-delivery)). |
| **Dev CLI** | `@incajs/cli` (Node) | Parent process during development. Owns the commands and wires its bundler adapter to its dev-protocol client. |
| **Bundler adapter** | `@incajs/cli`'s `adapter/vite` (Node) | Runs Vite in library/watch mode and announces each rebuild. The only part of the CLI that imports `vite`. |
| **Host client** | `@incajs/cli`'s `dev-client` (Node) | Launches and supervises the Rust host as a child and carries messages over its stdio. Depends on no bundler. |
| **Host bridge** | In-process Rust functions bound into the QuickJS context via `rquickjs` | Carries mutation operations (`createNode`, `setAttribute`, `appendChild`, ...) from JS to the Rust host. No C ABI or IPC is involved, since everything runs in one process. |

Vite supplies the Vue SFC compiler through `@vitejs/plugin-vue`.
Incarnative.js uses its library/build mode and its Runtime API, never its
browser dev server.

## System architecture

```
┌──────────────────────────────────────────────────────────────────┐
│        [ @incajs/cli — Node process (dev only, parent) ]         │
│  [ .vue / .tsx ] ──▶ [ adapter/vite ──▶ Vite (watch) ]           │
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
│  │   - `incajs` core (typed `__inca_native__` wrapper)        │  │
│  │   - Vite `ModuleRunner` + custom Transport/Evaluator       │  │
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

## HMR delivery

HMR uses Vite's Runtime API (`vite/module-runner`) instead of Vite's browser
client.

- The `ModuleRunner` runs inside QuickJS. Vite hands the evaluator
  `__vite_ssr_exports__` and `__vite_ssr_import__` as same-realm objects, so
  the runner runs beside the evaluator. Only `fetchModule` results and HMR
  payloads cross the boundary, as JSON.
- A custom `ModuleRunnerTransport` carries those messages between the Node
  process and the host over the stdio channel `dev-client` owns.
- A custom module evaluator runs the transformed module source inside
  QuickJS. Vite's SSR transform emits an async function body, and the
  evaluator builds and calls it with `new AsyncFunction(...)`.

This reuses Vite's module graph invalidation and its accept and dispose
boundaries, without a hand-rolled HMR protocol.
