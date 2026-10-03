<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Testing conventions

Where tests live, what kind of test goes where, and what must pass before a
change is done, for Rust and TypeScript.

## Rust (`crates/*`)

### Test placement

- **Unit tests**: `#[cfg(test)] mod tests` inline in the module they test
  (e.g. `crates/inca-gpui/src/tree.rs`, `crates/inca-bridge/src/bindings.rs`,
  `crates/inca-jsenv/src/engine.rs`).
- **Integration tests**: `tests/*.rs` in each crate, one file per
  cross-module concern (e.g. `crates/inca-gpui/tests/layout_parity.rs`).
  Cargo compiles each against the crate's public API only.
- **Window checks**: see [MANUAL_GUI_CHECK.md](./MANUAL_GUI_CHECK.md).
- **Tests that need both languages**: the root `tests/` package
  (`inca-tests`), never inside a crate or a package, so neither side depends
  on the other.
  - Rust files in `tests/tests/`: `config_contract.rs` starts Node.
    `js_core_integration.rs` reads `packages/core/dist`.
  - Node files in `tests/tests/` belong to `@incajs/e2e-tests`, which
    `pnpm test` does not run: `hmr-quickjs-state`, `host-startup-failure`,
    `host-shutdown` and `host-packaged-launch`. They spawn the real
    `inca-host` binary, and the first also starts a real Vite dev server.
  - `host-shutdown` speaks the dev protocol on raw stdio. On Linux with no
    display variable set it runs on gpui's headless platform. On macOS it
    opens a real window and needs a window-server session.
  - `host-packaged-launch` starts the host with no argv, from an unrelated
    working directory, with the bundle beside the executable. Its bundles
    open no window, so it needs no display.
- **Console tests**: console behaviour is unit-tested beside the code in
  `crates/inca-jsenv/src`. Timers are checked by the shape of the output.
  Colour is checked by exact SGR sequences and by stripping them.

### Required checks

All of the following must pass:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets`. `--all-targets` also compiles
  `examples/`, which has no automated test.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --exclude inca-tests`
- `pnpm -F incajs -F @incajs/cli build`, then `cargo test -p inca-tests`
- `cargo build -p inca-host`, then `pnpm -F @incajs/e2e-tests test`. CI runs
  the e2e step under `xvfb-run -a`.

### Toolchain pinning and MSRV

`rust-toolchain.toml` pins the toolchain, and rustup applies it to every
`cargo` call in the workspace, locally and on CI. `Cargo.toml`'s
`rust-version` is held equal to the pin. Bump both together. The resolver
(`resolver = "3"`) is MSRV-aware, so it does not pick a dependency that needs
a newer `rustc`.

### macOS coverage

The `rust-macos` job runs the same Rust steps as the Linux job, with
`inca-tests` excluded. The Node suites run on Linux only. The `plutil` tests
in `packages/cli/src/macos-app.test.mts` run only on macOS and are skipped
elsewhere.

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

- Each package's `vite.config.mts` excludes test files from `unplugin-dts`'s
  declaration scan (`dts({ include: ["src"], exclude: ["src/**/*.test.mts"] })`).
  Without it a co-located test file is published as `dist/*.test.d.mts`.
- Type-checking resolves workspace imports from source through the `"source"`
  export condition and `customConditions` in `tsconfig.base.json`. Vitest
  does not read `customConditions`, so a package that tests against another
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
  every run: `hmr.test.mts` calls Vite's `build()` in `beforeAll` with the
  package's `vite.config.mts`, overriding only `outDir`, `lib.entry` and
  `emptyOutDir`, and writes to a scratch directory.

## Running tests

- Whole workspace, from the root: `pnpm test`, `pnpm typecheck` (covers
  `examples/*` too) and `pnpm -r build`.
- One package: `pnpm --filter <pkg> test`, `typecheck` or `build`. A single
  package can miss what another package's tests catch.
- Coverage: `pnpm test:coverage`.
- Rust and the e2e test: see Required checks above. `pnpm test` does not run
  the e2e test.
