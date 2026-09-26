<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Testing conventions

Where tests live, what kind of test goes where, and what must pass before a
change is done — for both languages in this repo, kept as a matched pair.
Complements [GIT.md](./GIT.md)'s repo-wide rules the same way 
[FFI.md](./FFI.md) complements [ARCHITECTURE.md](./ARCHITECTURE.md).
Update this file as real conventions land, same as the other docs here.

## Rust (`crates/inca-gpui`)

### Test placement

- **Unit tests**: `#[cfg(test)] mod tests` inline in the module they test
  (e.g. `src/tree.rs`, `src/js/bindings.rs`, `src/js/engine.rs`).
- **Integration tests**: `tests/*.rs`, one file per cross-module concern
  (`tests/layout_parity.rs`, `tests/event_dispatch.rs`) — Cargo compiles
  each as its own crate against `inca-gpui`'s public API only, the same
  boundary a real external caller would see.
- **Manual/GUI checks**: tracked in
  [MANUAL_GUI_CHECK.md](./MANUAL_GUI_CHECK.md) instead of automated — see
  that doc for why the devcontainer can't do this alone and how to  actually
  run the check.
- **Tests that span both languages**: the root `tests/` package
  (`inca-tests`), never inside a crate or a package, so neither side
  depends on the other. Cargo picks up `tests/tests/*.rs` on its own; a
  file that needs the other language runs it (`config_contract.rs` starts
  Node) or reads what it built (`js_core_integration.rs` reads
  `packages/core/dist`).

### Required checks

All of the following must pass, not just `cargo test`:

- `cargo check --workspace --all-targets` — the `--all-targets` also
  compile-checks `examples/`, which has no automated test of its own (see
  `crates/inca-gpui/examples/hello_world.rs`'s doc comment)
- `cargo clippy --workspace --all-targets`
- `cargo fmt --all -- --check`
- `cargo test --workspace --exclude inca-tests` — the crates alone, which
  need no Node
- `cargo test -p inca-tests` — the root package, which does: run
  `pnpm --filter incajs build` first, or `js_core_integration` fails with a
  message saying so

`--workspace` covers a new crate automatically, the moment it exists.
`crates/inca-host` opens a window, which stays a manual check like
`crates/inca-gpui`'s own examples — see
[MANUAL_GUI_CHECK.md](./MANUAL_GUI_CHECK.md).

### Toolchain pinning and MSRV

`rust-toolchain.toml` pins the toolchain. rustup honours it for every
`cargo`/`rustup` call in this workspace, locally and on CI alike, so one
`rustc` builds everything. GitHub-hosted runner images ship a different
Rust version per OS and update on their own schedule, so the
`ubuntu-latest` and `macos-latest` legs disagree without it.

The pin declares `components` as well, so clippy and rustfmt arrive with
the toolchain. CI still runs `rustup component add` early, to download the
pinned toolchain before `Swatinem/rust-cache` derives its key from
`rustc -vV`. That download is expected on CI: the runner images don't carry
this exact version.

`Cargo.toml`'s `rust-version` is a separate declaration — the oldest
`rustc` this workspace supports — and is held equal to the pin. Bump both
together. It also feeds dependency resolution: `resolver = "3"` is
MSRV-aware and won't pick a dependency version needing a newer `rustc`.

The two can stay equal only while nothing outside this repo compiles
against these crates, which both `publish = false` settings currently
guarantee. Phase 13 ends that (see
[ROADMAP.md](./ROADMAP.md#phase-13-app-owned-rust-extensions-future)):
`rust-version` then drops below the pin and needs its own check, which
installs the floor toolchain and runs `cargo +<msrv> check`, the `+<msrv>`
overriding `rust-toolchain.toml`. That is a second toolchain, so a second
full gpui build under its own `Swatinem/rust-cache` key.

## TypeScript (`packages/*`)

### Test placement

Mirrors the Rust split above, using Vitest:

- **Unit tests**: `*.test.mts` co-located next to the module it tests
  (e.g. `packages/core/src/tree.test.mts` next to `src/tree.mts`),
  mocking `globalThis.__inca_native__`/`__inca_callbacks__` rather
  than driving a real QuickJS engine. A barrel (`src/index.mts`)
  re-exports only; its modules carry the tests.
- **Integration tests**: a `tests/` directory at the package root, for
  whatever a package's own unit tests can't reach mocked — e.g.
  `packages/core/tests/renderer.test.mts`, driving `createIncaApp`
  end-to-end against `incajs/vue` and a real (unmocked) `incajs` core.

### Required checks

**A change isn't done until root `pnpm test` passes with no warnings.**

Every package under `packages/*` defines the same four scripts, each
individually useful for checking just one thing directly:

- `lint` — `oxlint --type-aware`
- `format` — `oxfmt --check .`
- `typecheck` — `oxlint -A all --type-aware --type-check`
- `test` — `vitest run`

CI runs `lint`, `format`, and `typecheck` as their own separate steps,
not merely as a side effect of `test`.

Run `format` from the root, not from a package: `oxfmt.config.ts` — which
holds the import-sorting rules — lives there and only the root script
(`oxfmt --check --disable-nested-config .`) applies it. A package's own
`format` can pass on imports the root one rejects.

#### Agent-friendly lint output

`oxlint` takes `-f`/`--format=agent` (e.g. `oxlint --type-aware
--format=agent`) for plain, undecorated lines meant to be parsed rather
than read in a terminal.

### Build-tooling gotchas

Watch for these when scaffolding a new package too:

- Each package's `vite.config.mts` must exclude test files from
  `unplugin-dts`'s declaration scan
  (`dts({ include: ["src"], exclude: ["src/**/*.test.mts"] })`) —
  otherwise a co-located test file gets published too, as a stray
  `dist/*.test.d.mts`.
- `vitest.config`'s `test.passWithNoTests: true` treats a package with
  zero tests as passing rather than failing `pnpm -r test` — drop it
  again once real tests land (none of the current packages carry it).
- Each package's `exports` carries a `"source"` condition pointing at
  `src/index.mts`, and `tsconfig.base.json` sets
  `customConditions: ["source"]`, so type-checking resolves workspace
  imports from source instead of a built `dist/`. Vite/vitest don't read
  `customConditions`, so a package testing against another workspace
  package needs the same condition set explicitly, on both
  `resolve.conditions` and `ssr.resolve.conditions` (vitest resolves
  through Vite's SSR path) via its own `vitest.config.mts` merged on top
  of `vite.config.mts` — not currently needed by any package, since
  `incajs` and `@incajs/cli` each build entirely from their own source.
- A package's `tsconfig.json` `include` has to list every directory whose
  files are checked, plus a `*.mts` glob for its own root-level config
  files — a file outside `include` still gets linted, but under default
  compiler options rather than `tsconfig.base.json`'s, so `strict` and
  `customConditions` silently don't apply to it.
- Vitest replaces rather than merges an array option (`exclude`, etc.) with
  its default, so extending one means spreading `configDefaults` from
  `vitest/config` instead of retyping it — see the root `vitest.config.ts`.
- A test fixture spawned directly as a process (`packages/cli`'s
  `tests/dev-client/fixtures/*.mts`) needs its executable bit set. One
  added without it fails the test with `EACCES`, not a parse or
  module-resolution error.

## Running tests

- Whole workspace, from the repo root: `pnpm test` (all packages in one
  process, via `vitest.config.ts`'s `projects`) / `pnpm typecheck` (covers
  `examples/*` too) / `pnpm -r build`
- Single package, for iterating on one — may be incomplete on its own:
  `pnpm --filter <pkg> test` / `typecheck` / `build`
- Coverage: `pnpm test:coverage`, same run with `--coverage` added
- Rust: `cargo test -p inca-gpui` (see [AGENTS.md](../AGENTS.md#status)) —
  plus `cargo clippy`/`cargo fmt --check` from the Required checks list
  above, which aren't bundled into `cargo test` itself the way the root
  `pretest` bundles them on the TypeScript side.
