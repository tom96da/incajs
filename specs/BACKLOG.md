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

- **No app menu, so Cmd-Q does nothing on macOS**: GPUI builds the menu
  bar only when `Platform::set_menus` is called
  (`third_party/zed/crates/gpui_macos/src/platform.rs`), which
  `crates/inca-host/src/main.rs` never does. Closing the window and
  quitting from the Dock both work; the Quit item and its key equivalent
  are what's missing. Whether the menu is the host's to define or the
  app's to configure is undecided.

- **`dependabot.yml` is missing the `npm`/`cargo`/`github-actions`
  ecosystems**: it currently only auto-updates the devcontainer image/
  features.
