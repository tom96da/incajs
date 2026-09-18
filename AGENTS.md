<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project

**Incarnative.js** (`inca`) is an ultra-lightweight, Webview-free desktop application framework:

- **Engine / Core**: Rust, built on [`gpui`](https://www.gpui.rs/) for direct GPU rendering (no Chromium/DOM).
- **JS Runtime**: QuickJS via [`rquickjs`](https://github.com/DelSkayn/rquickjs), for a micro-sized, sub-second-startup runtime.
- **Frontend**: Vue 3 (first-class support, built first) via a custom renderer. React and other frameworks are a future, additive goal — not yet implemented, and not started until Vue 3 support is stable.
- **Bundler / dev tooling**: Vite, used in library/build mode (not as a browser dev server) — see [ARCHITECTURE.md](./specs/ARCHITECTURE.md).

See [README.md](./README.md) for the full pitch.

## Status

Phase 1 (the Rust host's FFI bridge core, now split across
`crates/inca-gpui`/`crates/inca-bridge`/`crates/inca-jsenv`) and Phase 2
(the pnpm workspace, `packages/core`, its `incajs/vue` Vue 3 custom
renderer, and two working `.vue` examples) are complete and visually
confirmed — [FFI.md](./specs/FFI.md) has the current binding vocabulary.

Phase 3 (the `inca` CLI, on the JS/TS side) is split into 3.1 through
3.4, with a `v0.0.1` release after 3.3. 3.1 (`inca dev`, full reload)
and 3.2 (`inca build`) are both done. 3.2 gained a Unit iii after the
fact: `inca-jsenv`'s engine had no module loader, so a build could only
ever emit one self-contained file, which silently broke a dynamic
`import()` or a `.vue` file's own `<style>` block. Fixed by giving the
engine a disk-backed loader and having the CLI report/copy every file a
build writes rather than one assumed `bundle.js`; the same fix let
`incajs/vue` move from a thin re-export of a separate `@incajs/vue`
package into an implementation inside `packages/core/src/vue`, the
shape Phase 2 always specified. Re-confirmed with a live launch on
macOS afterward. One item from 3.1 remains, not blocking 3.2 or 3.3: a
dev-only error panel drawn in the window, deliberately deferred — it
needs `position`/`z_index`/`overflow`, which don't exist before Phase 4.

3.3 is done. Unit ii: per-platform `@incajs/host-*` npm packages and
`inca package`, confirmed with a real launch on Linux and macOS. Unit
iii: `v0.0.1` published to npm via `.github/workflows/cd.yml`.
`darwin-x64` is excluded for now — no free Intel macOS CI runner exists;
`npm/darwin-x64/` stays as an unpublished placeholder.

See [PLAN.md](./specs/PLAN.md) for unit-by-unit detail on every phase
above.

Keep this section's status prose accurate as real logic lands — don't let it go stale.

## Architecture

- [ARCHITECTURE.md](./specs/ARCHITECTURE.md) — tech stack, system diagram, and how HMR is delivered into the embedded QuickJS runtime.
- [ROADMAP.md](./specs/ROADMAP.md) — the phased build-out plan (Vue 3 first, React later as an additive package).
- [FFI.md](./specs/FFI.md) — the JS↔Rust host bridge function surface.
- [PROTOCOL.md](./specs/PROTOCOL.md) — the dev protocol between `inca-host` and the Node process that spawns it.

### Guiding principles

- **Safety first**: Rust↔QuickJS bindings must handle pointer conversions and reference counts carefully — this boundary is the most likely source of memory leaks or segfaults.
- **Zero-overhead render loop**: don't run JS on every frame. JS executes only on reactivity updates, pushing snapshot mutations to Rust; Rust owns the retained tree and does layout/drawing natively.
- **Developer ergonomics**: frontend code stays strictly standard — `.vue`/`.tsx` code should feel identical to ordinary web development, not like it's targeting an embedded runtime.

## Repository structure

See [STRUCTURE.md](./specs/STRUCTURE.md) for the full directory map, including the pinned `third_party/` submodules (zed, rquickjs, quickjs-ng) and how to init/update them.

## Conventions

- **License**: dual-licensed MIT OR Apache-2.0 (see [LICENSE-MIT](./LICENSE-MIT) and [LICENSE-APACHE](./LICENSE-APACHE)). Prefix new source files with the SPDX header used at the top of this file (AGENTS.md):
  ```
  Copyright (c) <year> tom96da
  SPDX-License-Identifier: MIT OR Apache-2.0
  ```
- **Dev container**: `.devcontainer/Dockerfile` builds on `mcr.microsoft.com/devcontainers/rust:2-1-trixie`, adding the native build/runtime dependencies `gpui` needs (windowing, Vulkan, fontconfig — see the Dockerfile's comment), plus the `node` devcontainer feature. Use `pnpm` for any JS/frontend tooling — Vite's officially supported and tested runtime is Node.js, and this project's HMR bridge builds directly on Vite's less battle-tested Runtime API (`vite/module-runner`), so avoid introducing a second, less-proven runtime (e.g. Bun) there.
- **Git & commits**: see [GIT.md](./specs/GIT.md) for the commit message format and, most importantly, the review policy — never run `git commit`/`git commit --amend` without first showing the exact diff and message for explicit approval.
- **Testing & tooling**: see [TESTING.md](./specs/TESTING.md) for where tests live and the full set of checks (lint, format, type-check, tests) that must pass, for both Rust and TypeScript.
- Keep this file (not just README.md) up to date as real architecture, module boundaries, and commands land — this is the file agents read first.
