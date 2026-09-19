<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Architecture

Target architecture for Incarnative.js. This describes the design agents should
build toward. See [AGENTS.md](../AGENTS.md#status) for which layers already
match this design (the Rust host and the `incajs` core JS package) and which
are still forward-looking (the Vue custom renderer, the Vite/HMR bridge, and
everything after). Update it as each piece actually lands; don't let it drift
from reality.

The phased build-out of this design is tracked in [ROADMAP.md](./ROADMAP.md).
The JS↔Rust binding surface is specced in [FFI.md](./FFI.md).

## Tech stack

| Layer | Technology | Primary role |
| --- | --- | --- |
| **Native core** | Rust + [`gpui`](https://www.gpui.rs/) (wgpu) | Window management, event loop, retained virtual tree, direct GPU rendering. |
| **JS engine** | QuickJS via [`rquickjs`](https://github.com/DelSkayn/rquickjs) | Embedded, lightweight JS runtime executing UI logic and reactivity. |
| **Core JS package** | `incajs` (framework-agnostic) | Thin, typed JS wrapper around the host bridge (`__inca_native__`), shared by every framework adapter instead of duplicated in each. |
| **Frontend framework** | `incajs/vue` (first-class, current) / `incajs/react` (future, see [Roadmap](./ROADMAP.md#phase-10-react-custom-renderer-future)) | Custom renderer mapping virtual component trees to `incajs` calls — a subpath of the same package as the core, not a separate one, since neither adapter is ever imported without it. |
| **Bundler & dev tooling** | Vite, used in library/build mode (no browser dev server) | Compiles `.vue`/`.tsx` via the official `@vitejs/plugin-vue` (and later `@vitejs/plugin-react`); HMR is delivered through Vite's Runtime API instead of Vite's browser client — see [HMR delivery](#hmr-delivery). |
| **Dev CLI** | `@incajs/cli` (Node) | Parent process during development: owns the commands, and internally wires its own bundler adapter to its own dev-protocol client — see [Roadmap](./ROADMAP.md#phase-3-developer-tooling--hmr-integration) for why orchestration lives on the JS side. |
| **Bundler adapter** | `@incajs/cli`'s `adapter/vite` (Node) | Runs Vite in library/watch mode and announces each rebuild. The only part of the CLI that imports `vite`; a future bundler is a sibling adapter module inside the same package, not a sibling package — neither is ever imported on its own. |
| **Host client** | `@incajs/cli`'s `dev-client` (Node) | Launches and supervises the Rust host as a child and carries messages over its stdio — see [PROTOCOL.md](./PROTOCOL.md). Depends on no bundler, so bundler traffic rides the channel as a registered message name. |
| **Host bridge** | In-process Rust functions bound into the QuickJS context via `rquickjs` | Transfers mutation operations (`createNode`, `setAttribute`, `appendChild`, ...) from JS to the Rust host. Not a real C ABI or IPC boundary — everything runs in one process. |

Why Vite instead of a bare bundler (e.g. raw Rolldown): Vite owns the official,
maintained Vue SFC (and future React JSX) compiler integration
(`@vitejs/plugin-vue`). Reimplementing SFC compilation (template compile, CSS
extraction, source maps) on top of a bare bundler would be significant extra
work for no runtime benefit, since Incarnative.js never uses Vite's browser dev
server anyway — only its library/build API and its Runtime API (see below).
Modern Vite is also moving its own internals onto Rolldown, so the
Rust-bundler speed benefit isn't lost by choosing Vite.

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
│  │   - Vue 3 application (React: future, see Roadmap)         │  │
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

Vite ships a **Runtime API** (`vite/module-runner`, the same mechanism
`vite-node`/Vitest/Nuxt dev SSR use to run Vite-transformed modules outside a
browser) built specifically for "run Vite modules with HMR in a non-browser
environment." Incarnative.js uses it instead of Vite's browser client:

- The `ModuleRunner` itself runs **inside QuickJS**, not on the Node side.
  Vite hands the evaluator `__vite_ssr_exports__`/`__vite_ssr_import__` as
  same-realm objects, so a Node-side runner driving a QuickJS evaluator would
  turn every property access on a module namespace into a cross-process proxy
  hop. With the runner in QuickJS, only `fetchModule` results and HMR payloads
  cross the boundary, as JSON.
- A custom **`ModuleRunnerTransport`** carries those messages between the
  Node process and the host, over the channel `@incajs/cli`'s `dev-client` owns. A
  transport is just `invoke`, or `connect`+`send` — no WebSocket is required
  (Vite's own `createServerModuleRunnerTransport` is EventEmitter-only), so
  this rides the host child process's stdio.
- A custom **module evaluator** executes the transformed module source inside
  QuickJS. Vite's SSR transform emits an async *function body* taking the six
  `__vite_ssr_*` parameters, not an ES module, so no module loader is needed in
  QuickJS — but it does need an `AsyncFunction`-style entry point rather than
  the `Module::declare` path `inca build`'s output uses.

This gets real HMR (module graph invalidation, accept/dispose boundaries)
without reimplementing Vite's HMR protocol from scratch — the only new code is
the transport and the evaluator.

## Host bridge (FFI)

See [FFI.md](./FFI.md) for the exact function surface and the retained virtual
tree's node structure.

## Dev protocol (host ↔ dev-client)

See [PROTOCOL.md](./PROTOCOL.md) for the message surface the host and the Node
process that spawns it exchange over the child's stdio.
