<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Development

Project status, architecture and guiding principles for people and AI
agents working in this repository. Start with
[CONTRIBUTING.md](../CONTRIBUTING.md) for the rules every change follows.

## Project

**Incarnative.js** (`inca`) is an ultra-lightweight, Webview-free desktop application framework:

- **Engine / Core**: Rust, built on [`gpui`](https://www.gpui.rs/) for direct GPU rendering (no Chromium/DOM).
- **JS Runtime**: QuickJS via [`rquickjs`](https://github.com/DelSkayn/rquickjs), for a micro-sized, sub-second-startup runtime.
- **Frontend**: Vue 3 (first-class support, built first) via a custom renderer. React and other frameworks are a future, additive goal — not yet implemented, and not started until Vue 3 support is stable.
- **Bundler / dev tooling**: Vite, used in library/build mode (not as a browser dev server) — see [ARCHITECTURE.md](./ARCHITECTURE.md).

See [README.md](../README.md) for the full pitch.

## Status

Phase 1 (Rust host FFI bridge: `crates/inca-gpui`/`inca-bridge`/
`inca-jsenv`) and Phase 2 (pnpm workspace, `packages/core`, `incajs/vue`,
two `.vue` examples) are done — [FFI.md](./FFI.md) has the current
binding vocabulary.

Phase 3 (the `inca` CLI, `v0.0.1` released after 3.3) is done through 3.3:
`inca dev`, `inca build`, and `inca package` (per-platform
`@incajs/host-*` packages published via `cd.yml`; `darwin-x64` stays an
unpublished placeholder — no free Intel macOS runner). An app's settings
travel in its own build output — see [PROTOCOL.md](./PROTOCOL.md)
— and every app carries an application menu with a `Quit` item. 3.4
(HMR, `v0.0.6`) is functional and opt-in (`--experimental-hmr` /
`INCA_EXPERIMENTAL_HMR`; `inca dev` reloads the whole bundle by default
either way): a template-only edit preserves component state, a script
edit resets it, and a failed update is reported the same way a failed
full build already is. Known gaps live in
[BACKLOG.md](./BACKLOG.md), including a couple of `gpui`-layer
repaint bugs surfaced by real HMR editing that a full reload never hit.
The CLI prints the bundler's own output beside its own — see
[FAILURES.md](./FAILURES.md) for what it refuses and falls back
from. The host prints a full WHATWG `console` (all methods, format
specifiers, and `Date`, `RegExp`, `Map` and `Set` values), in colour on a
terminal (`NO_COLOR` and `FORCE_COLOR` apply).

Phase 4 (input & text editing) is running alongside 3.4: pointer input,
the focus model, keyboard input and scrolling (`overflow`, wheel) are
done. Text editing and IME, interactive visual state and `position`/
`z-index` are not built.

The current release is `v0.0.8`, on Node.js 22.18 or newer, zed v1.22.0
for `gpui` and rquickjs 0.14.0. Linux binaries are built in an
`ubuntu:22.04` container by both workflows, kept in sync, for a glibc
2.35 floor. The public docs site (`docs/`, VitePress — `guide/` for
prose, `reference/` for lookup) is deployed by
`.github/workflows/docs.yml` to
[tom96da.github.io/incajs](https://tom96da.github.io/incajs/).

See [PLAN.md](./PLAN.md) for unit-by-unit detail and deferred items
(e.g. 3.1's dev-only error panel, waiting on Phase 7's `position`/
`z-index`), and [BACKLOG.md](./BACKLOG.md) for gaps
outside the phased plan — several of them are places this framework and
`gpui` have drifted apart.

Keep this section's status prose accurate as real logic lands — don't let it go stale.

## Architecture

- [ARCHITECTURE.md](./ARCHITECTURE.md) — tech stack, system diagram, and how HMR is delivered into the embedded QuickJS runtime.
- [ROADMAP.md](./ROADMAP.md) — the phased build-out plan (Vue 3 first, React later as an additive package).
- [FFI.md](./FFI.md) — the JS↔Rust host bridge function surface.
- [PROTOCOL.md](./PROTOCOL.md) — the dev protocol between `inca-host` and the Node process that spawns it.

### Guiding principles

- **Safety first**: Rust↔QuickJS bindings must handle pointer conversions and reference counts carefully — this boundary is the most likely source of memory leaks or segfaults.
- **Zero-overhead render loop**: don't run JS on every frame. JS executes only on reactivity updates, pushing snapshot mutations to Rust; Rust owns the retained tree and does layout/drawing natively.
- **Developer ergonomics**: frontend code stays strictly standard — `.vue`/`.tsx` code should feel identical to ordinary web development, not like it's targeting an embedded runtime.

## Repository structure

See [STRUCTURE.md](./STRUCTURE.md) for the full directory map, including the pinned `third_party/` submodules (zed, rquickjs, quickjs-ng) and how to init/update them.

## Keeping docs current

Keep this file up to date as real architecture, module boundaries, and
commands land. The Status section in particular must not go stale.
