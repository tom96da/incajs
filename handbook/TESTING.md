<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Testing conventions

Where tests live, what kind of test goes where, and what must pass before a
change is done, for Rust and TypeScript.

## Rust (`crates/*`)

### Test placement

- **Unit tests**: an inline `#[cfg(test)] mod tests { .. }` at the end of the
  module they test. When the module and its tests together exceed 800 lines,
  the tests move to `<module>/tests.rs` (e.g.
  `crates/inca-gpui/src/tree/tests.rs`) and the module declares them with
  `#[cfg(test)] mod tests;`. A `tests.rs` that fits in 800 lines together with
  its module goes back inline. A module with many tests keeps shared helpers
  in `tests.rs` and the tests in `<module>/tests/<topic>.rs`. `tests.rs`
  declares its topic modules before its helpers. A file stays at or below 800
  lines.
- **Overlapping tests**: the branch is pinned once, where the code lives.
  When a unit test and an integration test pin the same branch, the unit
  test stays. An integration test pins one contract, such as what JS observes
  or how layers connect. A unit test that asserts only a private field gives
  way to the integration test of the same behaviour.
- **Shared helpers**: a helper used by tests of several modules of one crate
  lives once in that crate's `src/tests.rs`, declared in `lib.rs` with
  `#[cfg(test)] mod tests;`. `test_support` is the name of the `inca-bridge`
  feature and module, so it names nothing else.
- **Integration tests**: `tests/*.rs` in each crate, one file per
  cross-module concern (e.g. `crates/inca-gpui/tests/layout_parity.rs`).
  Cargo compiles each against the crate's public API only.
- **gpui behavior pins**: `crates/inca-gpui/tests/gpui_behavior.rs` checks the
  facts the event dispatch assumes about gpui (listener order, hit testing,
  focus on press, the wheel step on a hidden container, key propagation) on gpui's test platform. A
  gpui upgrade that changes one fails there first.
- **Opening a window**: see [MANUAL_GUI_CHECK.md](./MANUAL_GUI_CHECK.md).
- **Tests that need both languages**: the root `tests/` package, see
  [Root tests](#root-tests-tests).
- **Console tests**: console behaviour is unit-tested beside the code in
  `crates/inca-jsenv/src`. Timers are checked by the shape of the output.
  Color is checked by exact SGR sequences and by stripping them.

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
- Both name a test `<fn>::case_<N>_<id>`. `cargo test -p <crate> -- --list`
  prints the names.
- A loop whose iterations share state, or whose body only registers
  listeners for one dispatch, stays a loop.

### Lint allows

A test module that needs `clippy::unwrap_used` (or `float_cmp` and the like)
carries the allow itself, as an inner `#![allow(..)]` at the top of its inline
block, its `tests.rs`, its topic file or its integration test file. A
production file has no `#[allow]` on a `mod tests;` line, and a child module
does not rely on the allow of its parent.

### Required checks

All of the following must pass:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --exclude inca-tests`.
  `--all-targets` also compiles `examples/`, which has no automated test.
- `cargo clippy --workspace --all-targets --exclude inca-tests -- -D warnings`
- `cargo test --workspace --exclude inca-tests`

### Toolchain pinning and MSRV

`rust-toolchain.toml` pins the toolchain, and rustup applies it to every
`cargo` call in the workspace, locally and on CI. `Cargo.toml`'s
`rust-version` is held equal to the pin. Bump both together. The resolver
(`resolver = "3"`) is MSRV-aware, so it does not pick a dependency that needs
a newer `rustc`.

### macOS coverage

The `rust-macos` and `rust-macos-intel` jobs run the same Rust steps as the
Linux jobs, with `inca-tests` excluded. `test-macos` runs the Node unit suites.
`rust-node-macos` runs the same steps as `rust-node-ubuntu`, including
`cargo test -p inca-tests` and the `@incajs/e2e-tests` suite. The `plutil`
tests in `packages/cli/src/macosApp.test.mts` run only on macOS and are
skipped elsewhere.

## TypeScript (`packages/*`)

### Test placement

Mirrors the Rust split above, using Vitest:

- **Unit tests**: `*.test.mts` co-located next to the module it tests
  (e.g. `packages/core/src/tree.test.mts` next to `src/tree.mts`),
  mocking `globalThis.__inca_native__` and `__inca_callbacks__`. No real
  QuickJS engine runs. A barrel (`src/index.mts`)
  re-exports only; its modules carry the tests.
- **Integration tests**: a `tests/` directory at the package root, for
  whatever a package's own unit tests can't reach mocked — e.g.
  `packages/core/tests/renderer.test.mts`, driving `createIncaApp`
  end-to-end against `incajs/vue` and a real (unmocked) `incajs` core.

### Required checks

**A change isn't done until root `pnpm test` passes with no warnings.** CI
also runs `pnpm -r build`, which builds every workspace member that defines
a build script: the two packages, the examples and the docs site.

Every package under `packages/*` defines the same scripts:

- `lint`: `oxlint --type-aware`
- `format`: `oxfmt --check .`
- `typecheck`: `oxlint -A all --type-aware --type-check`
- `test`: `vitest run`

CI runs `lint`, `format` and `typecheck` as separate steps.

Run `format` from the root. `oxfmt.config.ts` holds the import-sorting rules,
and only the root script (`oxfmt --check --disable-nested-config .`) applies
them. A package's own `format` can pass on imports the root one rejects.

### Build-tooling gotchas

- Each package's `tsdown.config.mts` bundles the declarations from the entry
  files, so test files stay out of `dist` and no `dist/*.test.d.ts` is
  published.
- Type-checking resolves workspace imports from source through the `"source"`
  export condition and `customConditions` in `tsconfig.base.json`. Vitest
  does not read `customConditions`. A package that tests against another
  workspace package sets the condition on `resolve.conditions` and
  `ssr.resolve.conditions` in its own `vitest.config.mts`.
- A package's `tsconfig.json` `include` lists every directory whose files are
  checked, plus a `*.mts` glob for its root-level config files. A file outside
  `include` is still linted, but without `tsconfig.base.json`'s `strict` and
  `customConditions`.
- Vitest replaces an array option such as `exclude` with its default instead
  of merging. Spread `configDefaults` from `vitest/config` to extend it, as
  the root `vitest.config.ts` does.
- A test fixture spawned as a process (`packages/cli`'s
  `tests/dev-client/fixtures/*.mts`) needs its executable bit set.
- A test that needs its own package's build output produces it itself on
  every run. `hmr.test.mts` calls tsdown's `build()` in `beforeAll` with the
  package's `tsdown.config.mts`. It overrides only `entry`, `outDir` and
  `dts`, and writes to a scratch directory.

## Root tests (`tests/`)

The root `tests/` package holds the tests that need both languages. It sits
outside every crate and package, so neither side depends on the other. A test
that spawns the real `inca-host` binary lives here, never in a crate or a
package.

### Rust tests (`inca-tests`)

- Rust files in `tests/tests/`: `config_contract.rs` starts Node.
  `js_core_integration.rs` reads `packages/core/dist`.
- `tests/tests/ui/` mounts the `.vue` files in `tests/tests/ui-fixtures/` in
  the `Harness`. `build-ui-fixtures.mjs` bundles them all once per run and
  needs `packages/core/dist`. Only top-level `.vue` files become fixtures;
  helper components live in `ui-fixtures/parts/`.
- The **`test-support` feature** gates `inca-host`'s `Harness` and `snapshot`
  and enables gpui's test platform. The root `tests/` dev-dependency turns it
  on. Its unit tests are in `crates/inca-host/src/harness/tests/` and
  `snapshot.rs`.
- The **`test-support` feature** of `inca-bridge` exposes `test_support`, the
  engine helpers shared by its unit and integration tests. The crate's own
  dev-dependency turns it on.

### Node e2e tests (`@incajs/e2e-tests`)

The Node files in `tests/tests/` belong to `@incajs/e2e-tests`, which
`pnpm test` does not run: `hmr-quickjs-state`, `hmr-timers`,
`host-startup-failure`, `host-startup-wait`, `host-shutdown`,
`host-packaged-launch`, `host-print-config`, `host-source-map`, `host-timers`,
`hmr-callback-ids`, `host-stack-limit` and `inca-config`.
They spawn the real `inca-host` binary, and `hmr-quickjs-state`, `hmr-timers`
and `hmr-callback-ids` also start a real Vite dev server.

- `tests/support/headless.mts` is a vitest setup file. It removes `DISPLAY`
  and `WAYLAND_DISPLAY`, so on Linux every spawned host runs on gpui's
  headless platform and the tests need no X server. A new test needs no code
  for this.
- `tests/support/hostBin.mts` exports `resolveTestHostBin()`, which returns
  `target/debug/inca-host`, else `target/release/inca-host`. The tests that
  spawn the host import it.
- `host-shutdown` speaks the dev protocol on raw stdio. On macOS it opens a
  real window and needs a window-server session.
- `host-startup-wait` starts the host on entries whose top-level `await`
  stays pending and checks when `ready` arrives. Its tests run together and
  wait out the 2 seconds in real time. Other tests that depend on that wait
  use gpui's fake clock in `inca-host`.
- `host-print-config` runs `--print-config` and compares the printed JSON and
  the exit code.
- `host-timers` runs an interval and a timeout of the built `incajs` on the
  real clock.
- `host-stack-limit` runs a script that recurses until the stack limit throws
  and checks that the host exits with 1 and no signal.
- `hmr-timers` edits a script under experimental HMR and checks that a timer
  the old module started keeps ticking.
- `host-packaged-launch` starts the host with no argv, from an unrelated
  working directory, with the bundle beside the executable. Its bundles open
  no window, so it needs no display.

### Required checks

All of the following must pass:

- `pnpm -F incajs -F @incajs/cli build`, then
  `cargo clippy -p inca-tests --all-targets -- -D warnings` and
  `cargo test -p inca-tests`
- `cargo build -p inca-host`, then `pnpm -F @incajs/e2e-tests test`. CI runs
  the e2e step under `xvfb-run -a`.

On CI, only `rust-node-ubuntu` lints and type-checks `tests/`.

## Running tests

- Whole workspace, from the root: `pnpm test`, `pnpm typecheck` (covers
  `examples/*` too) and `pnpm -r build`.
- One package: `pnpm --filter <pkg> test`, `typecheck` or `build`. A single
  package can miss what another package's tests catch.
- Coverage: `pnpm test:coverage`.
- Rust and the root tests: see the Required checks of each section.
