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

### Done

- **Host**: the Rust FFI bridge (`inca-gpui`, `inca-bridge`, `inca-jsenv`),
  with a `console`.
- **Packages**: the pnpm workspace, `packages/core`, `incajs/vue` and the
  `.vue` examples.
- **CLI**: `inca dev`, `inca build` and `inca package`, with per-platform
  `@incajs/host-*` packages published by `cd.yml`.
- **HMR**: opt-in through `--experimental-hmr` or `INCA_EXPERIMENTAL_HMR`.
  A template edit keeps component state and a script edit resets it.
- **Input**: pointer, focus, keyboard and scrolling (`overflow`, wheel).

### Not started

- Text editing and IME.
- Interactive visual state.
- `position` and `z-index`.
- React and other frontends. Vue 3 has to be stable first.

### Foundation

- **Release**: `v0.0.8`.
- **Node.js**: 22.18 or newer for the CLI and tooling.
- **Toolchain**: zed `v1.22.0` for `gpui`, rquickjs `0.14.0`.
- **Linux binaries**: built in an `ubuntu:22.04` container by both workflows,
  for a glibc 2.35 floor.
- **macOS**: the `darwin-x64` package is in preparation and not published yet.
- **Docs site**: VitePress in `docs/`, deployed by `.github/workflows/docs.yml`
  to [incajs.tom96da.com](https://incajs.tom96da.com/).

## Architecture

An app is a Vue 3 project. Vite compiles it in library mode, and a QuickJS
runtime inside a Rust host runs the bundle. A custom renderer turns Vue's
reactive updates into calls over an in-process bridge. The host keeps a
retained tree, lays it out and draws it on the GPU through `gpui`. JS runs
only when reactive state changes, never per frame.

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
- **Zero-overhead render loop**: don't run JS on every frame. JS executes
  only on reactivity updates. It pushes snapshot mutations to Rust. Rust
  owns the retained tree and does layout and drawing natively.
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
- [MANUAL_GUI_CHECK.md](./MANUAL_GUI_CHECK.md): the checks that need a window.
- [GIT.md](./GIT.md): commit format and the review policy.
