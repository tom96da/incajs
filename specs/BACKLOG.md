<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Backlog

Desktop-app-shaped gaps and quality-of-life fixes that fall outside
[ROADMAP.md](./ROADMAP.md)'s phased plan — things noticed while building,
not scheduled work. Anything here can be picked up whenever, in any order,
independent of the phase currently in progress.

Once a project needs external visibility or outside contribution, this
should move to GitHub Issues instead; for now, a single Markdown file is
enough overhead for a single maintainer plus AI pairing.

- **Vue template type-checking**: `vue-tsc` mechanically works (verified
  against a vanilla `create-vue` scaffold), but it wouldn't catch anything
  in this repo's `.vue` files today. There's no `JSX.IntrinsicElements`/
  `GlobalComponents` typing wiring `packages/core`'s `StyleProps`
  (`packages/core/src/types.mts`) into Vue's template type system, so a
  `:style` binding or a `v-for` has nothing to check against. Declaring
  that typing is the prerequisite; only after that does adding `vue-tsc`
  (plus the full `vue` package, `@vue/tsconfig`) actually buy anything.

- **Rich console output for `inca dev`/`build`**
  ([tom96da/incajs#1](https://github.com/tom96da/incajs/issues/1)): replace
  the current plain `[inca] ...` lines with `consola`-based output — a
  build summary (file, size, duration) and a dev-server log naming which
  file triggered a rebuild. Report the changed file and rebuild duration
  through `@incajs/cli`'s `watch()` (`onBuild`/`onError`, via the
  underlying watcher's already-available `"change"` event). Decided
  against `@clack/prompts` here — that fits interactive scaffolding
  prompts, not a running log stream; save it for a future bootstrap CLI.
  Per-module HMR output ("hot update: App.vue") is out of scope, blocked on
  Phase 3.4.

- **Linux `.so` bundling in `inca package`**: decide how `inca package`
  ships native `.so` dependencies on Linux, or sidesteps needing to via
  `RUST_FONTCONFIG_DLOPEN=1`/`FREETYPE2_NO_PKG_CONFIG=1`.

- **`strip = true` for the release profile**: shrink release binaries by
  stripping symbols.

- **QuickJS bytecode precompilation**: precompile JS to QuickJS bytecode
  ahead of time instead of parsing source at startup.

- **`.vue` `<style>` blocks don't reach the screen**: scoped/global
  `<style>` blocks in `.vue` SFCs currently have no effect through the
  custom renderer.

- **`dependabot.yml` is missing the `npm`/`cargo`/`github-actions`
  ecosystems**: it currently only auto-updates the devcontainer image/
  features.
