<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Development

The entry point to the handbook: where the project stands and where each
document lives. Read [CONTRIBUTING.md](../CONTRIBUTING.md) first for the
rules every change follows.

**Incarnative.js** (`inca`) is a Webview-free desktop application framework.
Rust and [`gpui`](https://www.gpui.rs/) render on the GPU, QuickJS runs the
JS, and a custom renderer drives it from Vue 3. See [README.md](../README.md)
for the full pitch.

## Status

History is in [PLAN.md](./PLAN.md) and [CHANGELOG.md](../CHANGELOG.md).

### Recently done

- **Nodes**: tree links, the mutation members, fragments, cloning and the
  relations between nodes. Template elements hold their content, and HTML
  strings parse into the tree through `innerHTML` and `insertAdjacentHTML`,
  and serialize back out through `outerHTML`.
- **Styles**: `<style>` elements, `<style>` blocks, scoped styles and CSS
  imports apply, and `:hover`, `:active` and `:focus` match. A style edit
  keeps component state under HMR.
- **Built-ins**: the timers, `requestAnimationFrame()`, `URL` and
  `performance.now()`. `incajs/globals` declares the types of every built-in
  global.

### In progress

- **A base for Vue Vapor mode**: `document` as a node of the tree, the
  `Node`, `Element` and `Text` interfaces, and the attribute, class and
  style members that Vapor reads.
- **Built-ins as the web expects them**: definitions written one way, so that
  libraries find the globals they probe.

### Not started

- `textarea`, word-wise editing, undo and redo, and the selection API.
- `position` and `z-index`.
- The `html` and `body` elements, namespaces and a global `window`.
- React and other frontends. Vue 3 has to be stable first.

### Foundation

- **Release**: `v0.0.31`.
- **Node.js**: 22.18 or newer.
- **Toolchain**: zed `v1.24.3` for `gpui`, rquickjs `0.14.0`.
- **Linux binaries**: built in an `ubuntu:22.04` container by both workflows,
  for a glibc 2.35 floor.
- **macOS**: host packages for arm64 and x64 (Intel).
- **Docs site**: VitePress in `docs/`, deployed by `.github/workflows/docs.yml`
  to [incajs.tom96da.com](https://incajs.tom96da.com/).

## Architecture

An app is a Vue 3 project. Vite compiles it in library mode, and a QuickJS
runtime inside a Rust host runs the bundle. A custom renderer turns Vue's
reactive updates into calls over an in-process bridge. The host keeps a
retained tree, lays it out and draws it on the GPU through `gpui`. JS runs
when reactive state changes, when a frame frees a node and when a
timer is due.

During development, the `inca` CLI runs as a Node parent process. It bundles
with Vite, launches the host as a child and sends rebuilds and HMR updates
over the child's stdio.

- [ARCHITECTURE.md](./ARCHITECTURE.md) describes the layers, the system
  diagram and how HMR reaches the embedded runtime.
- [FFI.md](./FFI.md) lists the functions, tags and style props JS can call on
  the host.
- [PROTOCOL.md](./PROTOCOL.md) defines the messages between the host and the
  Node process that spawns it.
- [FAILURES.md](./FAILURES.md) records what the CLI refuses and what it falls
  back from.

### Guiding principles

- **Safety first**: Rust↔QuickJS bindings must handle pointer conversions
  and reference counts carefully. This boundary is the most likely source
  of memory leaks or segfaults.
- **JS values stay in JS**: Rust keeps ids and plain data. A handle kept
  past the call that produced it outlives its context, so functions and
  objects live on the JS side and Rust refers to them by id. Listener
  callbacks are the model, see [FFI.md](./FFI.md#js-values-stay-in-js).
- **Zero-overhead render loop**: don't run JS on every frame. JS executes
  on reactivity updates, when a frame frees a node and when a timer is due.
  It pushes snapshot mutations to Rust. Rust owns the retained tree and does
  layout and drawing natively.
- **Developer ergonomics**: frontend code stays strictly standard. `.vue`
  and `.tsx` code should feel identical to ordinary web development.

## Planning

- [ROADMAP.md](./ROADMAP.md): the phased build-out, Vue 3 first and React later.
- [PLAN.md](./PLAN.md): unit-by-unit tasks and deferred items.
- [BACKLOG.md](./BACKLOG.md): gaps outside the phased plan.

## Repository

[STRUCTURE.md](./STRUCTURE.md) maps the directories, including the pinned
`third_party/` submodules and how to initialise them.

## Quality

- [TESTING.md](./TESTING.md): where tests live and the checks that must pass.
- [MANUAL_GUI_CHECK.md](./MANUAL_GUI_CHECK.md): opening a window and fixing
  its start-up.
- [GIT.md](./GIT.md): commit format and the review policy.
