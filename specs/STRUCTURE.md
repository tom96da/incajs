<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Repository structure

A map of this repository for AI agents. Read [AGENTS.md](../AGENTS.md) first — this file is the detailed reference it points to.

```
incajs/
├── README.md                # public-facing project pitch (features, tech stack, license)
├── LICENSE-MIT
├── LICENSE-APACHE           # dual-licensed MIT OR Apache-2.0
├── AGENTS.md                # agent instructions entry point — read this first
├── CLAUDE.md                # Claude Code entry point; just `@AGENTS.md`
├── Makefile                 # generates the root .gitignore from .gitignore.d/*.gitignore
├── .gitignore.d/            # per-topic gitignore fragments (Node/Rust/common) concatenated by `make .gitignore` — edit these, never .gitignore directly
├── specs/
│   ├── STRUCTURE.md         # this file
│   ├── ARCHITECTURE.md      # target tech stack, system diagram, HMR delivery design
│   ├── ROADMAP.md           # planned phased implementation (Vue 3 first, React later)
│   ├── FFI.md               # JS↔Rust host bridge function surface
│   ├── PROTOCOL.md          # dev protocol between the host binary and the Node process spawning it
│   ├── PLAN.md              # checkbox-tracked, per-phase task breakdown of ROADMAP.md
│   ├── BACKLOG.md           # unscheduled desktop-app gaps and QoL fixes, outside ROADMAP.md's phased plan
│   ├── GIT.md               # commit message format and the commit-review policy
│   ├── TESTING.md           # test placement and required checks, Rust + TypeScript
│   └── MANUAL_GUI_CHECK.md  # how to visually verify a GPUI window/example yourself
├── .devcontainer/
│   ├── devcontainer.json    # builds from ./Dockerfile, adds the `node` and `claude-code` features
│   ├── Dockerfile           # extends the Rust devcontainer image with gpui's native build deps
│   └── post-create.sh       # post-create ownership fixes (non-image-layer setup only)
├── .github/
│   ├── dependabot.yml       # auto-updates the devcontainer image/features only, for now
│   └── workflows/
│       ├── ci.yml           # runs TESTING.md's required checks on push/PR
│       ├── cd.yml           # tags/publishes to npm and GitHub Releases once CI passes on main
│       └── docs.yml         # builds docs/ and deploys it to GitHub Pages
├── Cargo.toml               # Rust workspace manifest
├── Cargo.lock               # locked Rust dependency graph, including git-pinned gpui
├── crates/
│   ├── inca-gpui/           # Rust host: retained tree, QuickJS bridge, GPUI render (Phase 1, done)
│   ├── inca-bridge/         # binds a QuickJS realm to the retained tree — the __inca_native__ bridge and event dispatch (Phase 1, done)
│   ├── inca-jsenv/          # the host objects installed into the QuickJS realm — console today, more in Phase 6
│   └── inca-host/           # the runtime binary: loads a bundle and opens the window (Phase 2 Unit iv, done; grows a dev mode in Phase 3.1)
├── pnpm-workspace.yaml       # pnpm workspace member globs (packages/*, examples/*, npm/*, docs)
├── package.json              # root workspace manifest — lint/format/typecheck/test/build scripts
├── pnpm-lock.yaml
├── tsconfig.base.json         # shared TS compiler options, extended by each package's tsconfig.json
├── oxlint.config.ts / oxfmt.config.ts  # shared lint/format config for all TS packages
├── packages/
│   ├── core/                # `incajs` — framework-agnostic host bridge wrapper (Phase 2 Unit i–ii, done), plus `incajs/vue`, its Vue 3 custom renderer subpath (Phase 2 Unit iii, done)
│   └── cli/                  # `@incajs/cli` — the `inca` dev/build/package CLI (Phase 3.1 Unit vi, done); internally absorbs the dev-protocol client (Phase 3.1 Unit iv, done) and the Vite bundler adapter (Phase 3.1 Unit v, done), neither ever imported on its own
├── npm/                      # per-platform npm packages carrying a prebuilt inca-host each (Phase 3.3 Unit i)
│   ├── darwin-arm64/
│   ├── darwin-x64/
│   ├── linux-arm64/
│   └── linux-x64/
├── examples/
│   ├── hello_world/         # Vue port of crates/inca-gpui/examples/hello_world.rs (Phase 2 Unit iv, done)
│   └── click_counter/       # Vue port of crates/inca-gpui/examples/click_counter.rs (Phase 2 Unit iv, done)
├── docs/                     # the public VitePress site, deployed by .github/workflows/docs.yml
└── third_party/             # pinned upstream sources, as git submodules — see below
    ├── zed/                 # zed-industries/zed @ v1.17.2
    ├── rquickjs/            # DelSkayn/rquickjs @ v0.12.2
    │   └── sys/quickjs/     # nested submodule: quickjs-ng @ the commit rquickjs v0.12.2 pins
    ├── vue/                 # vuejs/core @ v3.5.42
    └── vite/                # vitejs/vite @ v8.2.2
```

## Status

See [AGENTS.md](../AGENTS.md#status) for what has landed so far and what
hasn't, and [TESTING.md](./TESTING.md) for the required checks (Rust and
TypeScript) — not restated here, to avoid drifting out of sync.

Update this file and [AGENTS.md](../AGENTS.md) as real crates, packages, and ownership boundaries land — do not let either go stale.

## Target workspace layout (full plan)

Everything already built appears in the tree above. This shows only what
[ROADMAP.md](./ROADMAP.md)'s later phases still add — nested under the
existing `crates/`/`packages/` directories shown above:

```
incajs/
├── crates/
│   └── inca-macros/         # host bridge binding helper macros
└── packages/
    └── vite-runtime/        # `@incajs/vite-runtime` — Vite Runtime API integration, runs inside QuickJS (Phase 3.4)
```

React lands as `incajs/react` — a new subpath inside `packages/core`, alongside `incajs/vue`, not a new top-level package (Phase 10).

Move an entry up into the tree above once it actually lands, per
[ROADMAP.md](./ROADMAP.md).

## `third_party/` — pinned upstream sources

These are **git submodules pinned to a specific tagged release commit**, not moving branches. Their purpose right now is reference/study material during development (e.g. reading how `gpui` or `rquickjs` implement something) — whether any of this ends up wired into the actual build (git submodule vs. crates.io dependency) is still undecided.

| Path | Upstream | Pinned at | Why it's here |
|---|---|---|---|
| `third_party/zed` | [zed-industries/zed](https://github.com/zed-industries/zed) | `v1.17.2` | Source of the `gpui` crate this project builds its rendering on. |
| `third_party/rquickjs` | [DelSkayn/rquickjs](https://github.com/DelSkayn/rquickjs) | `v0.12.2` | Rust bindings to QuickJS this project uses as its JS runtime. |
| `third_party/rquickjs/sys/quickjs` | [quickjs-ng/quickjs](https://github.com/quickjs-ng/quickjs) | commit pinned by rquickjs `v0.12.2` | rquickjs's own nested submodule — the actual QuickJS engine (the actively-maintained `quickjs-ng` fork, not Bellard's original `bellard/quickjs`). Not the same tree as `bellard/quickjs`; add that separately if it's ever needed for comparison. |
| `third_party/vue` | [vuejs/core](https://github.com/vuejs/core) | `v3.5.42` | Reference source for `@vue/runtime-core`'s `createRenderer`/`RendererOptions` API and `runtime-dom`'s reference `nodeOps`/`patchProp` implementation — used while building `incajs/vue`'s custom renderer (Phase 2). Matches the version `packages/core/package.json` already depends on. |
| `third_party/vite` | [vitejs/vite](https://github.com/vitejs/vite) | `v8.2.2` | Reference source for Vite's Runtime API (`vite/module-runner`: `ModuleRunner`, `ModuleRunnerTransport`, `ModuleEvaluator`), whose published docs are thin — used while building Phase 3's HMR bridge. Matches the version the root `package.json` already depends on. |

All are registered **shallow** (`submodule.<name>.shallow = true` in the relevant `.gitmodules`) since full history is large and irrelevant here.

- Clone this repo with submodules: `git clone --recurse-submodules <url>` (respects shallow settings).
- Init/update after a plain clone: `git submodule update --init --depth 1 --recursive` — `--recursive` is required to reach `third_party/rquickjs/sys/quickjs`, since it's registered in rquickjs's own `.gitmodules`, not this repo's.
- `third_party/rquickjs/sys/quickjs/test262` (the ECMA-262 conformance test suite, very large) is intentionally left uninitialized — it's not needed here.
- To bump a pin: `cd` into the submodule, `git fetch --depth 1 origin <new-tag> && git checkout FETCH_HEAD`, then commit the updated gitlink from the superproject. Always pin to a tag/commit, never track a branch.
