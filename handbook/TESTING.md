<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Testing conventions

Where tests live, what kind of test goes where, and what must pass before a
change is done, for Rust and TypeScript.

## Rust (`crates/*`)

### Test placement

- **Unit tests**: an inline `#[cfg(test)] mod tests { .. }` at the end of the module they test. A
  file stays at or below 800 lines. Past that, split the module by concern. Each new module keeps
  its code and its tests in one file.
- **Shared helpers**: a helper used by tests of several modules of one crate lives once in that
  crate's `src/tests.rs`.
- **Integration tests**: `tests/*.rs` in each crate, one file per cross-module concern. Cargo
  compiles each against the crate's public API only.
- **Overlapping tests**: the branch is pinned once, where the code lives. When a unit test and an
  integration test pin the same branch, the unit test stays. An integration test pins one contract,
  such as what JS observes or how layers connect.

### Test names

A test name is a snake_case sentence of the behaviour, with the subject
first. It has no `test_` prefix and at most 80 characters. It says `throws` for
a JS exception and uses US spelling (`canceled`, `centers`).

### Table tests

Independent cases of one behaviour are one test per case.

- A `#[test]` uses `rstest`: `#[rstest]` with `#[case::<id>(..)]`.
- A `#[gpui::test]` uses `#[gpui_cases]` from `inca-test-macros`, above
  `#[gpui::test]`, with `#[case::<id>(..)]`. `cx: &mut TestAppContext` stays
  the first parameter and the last parameters take the case values.
  `rstest` cannot supply `cx`.
- Both name a test `<fn>::case_<N>_<id>`.
- A loop whose iterations share state, or whose body only registers
  listeners for one dispatch, stays a loop.

### Lint allows

A test module that needs `clippy::unwrap_used` (or `float_cmp` and the like) carries the allow
itself, as an inner `#![allow(..)]` at the top of its inline block, the shared `tests.rs` or its
integration test file. A production file has no `#[allow]` on a `mod tests;` line, and a child
module does not rely on the allow of its parent.

### Required checks

All of the following must pass:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --exclude inca-tests`.
  `--all-targets` also compiles `examples/`, which has no automated test.
- `cargo clippy --workspace --all-targets --exclude inca-tests -- -D warnings`
- `cargo test --workspace --exclude inca-tests`

## TypeScript (`packages/*`)

### Test placement

- **Unit tests**: `*.test.mts` next to the module it tests, mocking `globalThis.__inca_native__`. No
  real QuickJS engine runs under `pnpm test`. A barrel (`src/index.mts`) re-exports only, and its
  modules carry the tests.
- **Integration tests**: a `tests/` directory at the package root, for what a package's unit tests
  can't reach with mocks.
- **Tests that need both languages**: the root `tests/` package, see
  [Cross-language tests](#cross-language-tests-tests).
- **QuickJS run**: `pnpm -F incajs test:quickjs [<file>...]` runs the test files of `packages/core`,
  `src/**/*.test.mts` and `tests/*.test.mts`, in QuickJS through `vitest-pool-quickjs`, configured
  in `vitest.quickjs.config.mts`. The run is not part of `pnpm test`.

### Required checks

**A change isn't done until root `pnpm test` passes with no warnings.** A run of one package
(`pnpm --filter <pkg> test`) can miss a failure in the tests of another package that uses it. CI
also builds every workspace member except the docs site.

Every package under `packages/*` defines the same scripts:

- `lint`: `oxlint --type-aware`
- `format`: `oxfmt --check .`
- `typecheck`: `tsc -b --noEmit`
- `test`: `vitest run`

CI runs `lint`, `format` and `typecheck` as separate steps.

Run `format` from the root. `oxfmt.config.ts` holds the import-sorting rules,
and only the root script (`oxfmt --check --disable-nested-config .`) applies
them. A package's own `format` can pass on imports the root one rejects.

### Build output

- A test that needs its own package's build output produces it itself on
  every run, into a scratch directory.

## Cross-language tests (`tests/`)

The root `tests/` package holds the tests that need both languages. It sits outside every crate and
package, so neither side depends on the other. A test that spawns the real `inca-host` binary lives
here, never in a crate or a package.

### Test placement

- **Rust tests (`inca-tests`)**: `tests/tests/*.rs`. The UI tests in `tests/tests/ui/` mount the
  `.vue` files in `tests/tests/ui-fixtures/` in the `Harness`. Only top-level `.vue` files become
  fixtures, and helper components live in `ui-fixtures/parts/`.
- **Node e2e tests (`@incajs/e2e-tests`)**: `tests/tests/*.test.mts`. They spawn the real
  `inca-host` binary, and `pnpm test` does not run them.

### Test support

- `tests/support/headless.mts` is a vitest setup file. It removes `DISPLAY` and `WAYLAND_DISPLAY`,
  so on Linux every spawned host runs on gpui's headless platform and the tests need no X server. A
  new test needs no code for this.
- `tests/support/hostBin.mts` exports `resolveTestHostBin()`, which returns
  `target/debug/inca-host`, else `target/release/inca-host`. The tests that spawn the host import
  it.
- The `test-support` feature of `inca-host` gates its `Harness` and `snapshot` and enables gpui's
  test platform. The `test-support` feature of `inca-bridge` exposes the engine helpers its tests
  share. A dev-dependency turns each on.

### Required checks

All of the following must pass:

- `pnpm -F incajs -F @incajs/cli build`, then
  `cargo clippy -p inca-tests --all-targets -- -D warnings` and `cargo test -p inca-tests`
- `cargo build -p inca-host`, then `pnpm -F @incajs/e2e-tests test`

On CI, the Rust & Node jobs lint, format-check and type-check `tests/`.

## CI

[`ci.yml`](../.github/workflows/ci.yml) runs the checks of this document on every pull request and
on pushes to the integration and release branches. A job runs only when the files of its area
changed. One status check stands for all jobs.

- **Lint, format and type check**: on one Node.js version.
- **Node.js and Bun**: the TypeScript unit tests on Node.js 22, 24 and 26 on Linux, on Node.js 26 on
  macOS, and on Bun on Linux.
- **Rust**: on Linux (x64 and arm64) and on macOS (arm64 and x64). Linux jobs run in the oldest
  supported Ubuntu.
- **Rust and JavaScript together**: the cross-language tests on Linux and macOS with Node.js 26, and
  on Linux with Bun.

A test that only macOS can run skips itself elsewhere.
