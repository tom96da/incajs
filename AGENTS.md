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

Phase 1 (Rust host FFI bridge: `crates/inca-gpui`/`inca-bridge`/
`inca-jsenv`) and Phase 2 (pnpm workspace, `packages/core`, `incajs/vue`,
two `.vue` examples) are done — [FFI.md](./specs/FFI.md) has the current
binding vocabulary.

Phase 3 (the `inca` CLI) is split into 3.1–3.4, with `v0.0.1` released
after 3.3. 3.1 (`inca dev`), 3.2 (`inca build`, with a file-backed module
loader so a build can emit more than one file) and 3.3 (`inca package`,
per-platform `@incajs/host-*` packages published via `cd.yml`) are done.
`darwin-x64` stays an unpublished placeholder — no free Intel macOS runner.

3.4 (HMR) is in progress: only `inca.config.ts` is done — app metadata,
build settings and the window, `defineConfig` from `@incajs/cli/config`,
deprecating `package.json`'s `"inca"` key. `inca dev` still reloads the
whole bundle.

An app's settings travel in its own build output: `inca build` writes
`inca.json` beside the entry, and the host reads its name, identifier and
window before opening one — see [PROTOCOL.md](./specs/PROTOCOL.md). On
macOS `inca dev` also assembles a `.app` under `node_modules/.inca`, since
the Dock and the Finder read a name and an icon from a bundle and nowhere
else. Every app carries an application menu with a `Quit` item.

The CLI prints the bundler's own output beside its own. See
[FAILURES.md](./specs/FAILURES.md) for what it refuses, what it falls back
from, and where each is reported.

The current release is `v0.0.4`, on Node.js 22.18 or newer, zed v1.20.2
for `gpui` and rquickjs 0.14.0. Its Linux binaries are built in an
`ubuntu:22.04` container by both workflows, fixing their glibc floor at
2.35 — change that image in one file and the other has to follow. The
public docs site (`docs/`, VitePress) — `guide/` for prose, `reference/`
for lookup — is deployed by `.github/workflows/docs.yml` to
[tom96da.github.io/incajs](https://tom96da.github.io/incajs/).

See [PLAN.md](./specs/PLAN.md) for unit-by-unit detail and deferred items
(e.g. 3.1's dev-only error panel, waiting on Phase 4's `position`/
`z-index`/`overflow`), and [BACKLOG.md](./specs/BACKLOG.md) for gaps
outside the phased plan — several of them are places this framework and
`gpui` have drifted apart.

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
