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

- **Linux runtime dependencies in `inca package`**: measured on an
  `aarch64` release build — 8 direct `NEEDED` libraries, desktop-typical
  except `libxkbcommon-x11`. Vulkan and Wayland are `dlopen`ed, and a
  Vulkan ICD is GPU-specific, so neither can be bundled meaningfully. The
  binary carries no `RPATH`/`RUNPATH`, so bundling anything means adding
  an `$ORIGIN` rpath (as `third_party/zed/script/bundle-linux` does) or a
  launcher first. `FREETYPE2_NO_PKG_CONFIG=1` drops `libfreetype` by
  building it statically; `RUST_FONTCONFIG_DLOPEN=1` only defers the
  failure to runtime. Whether this is worth doing at all is the question.

- **The documented Linux dependencies are wider than the binary's**: in
  v0.0.3's published binaries FreeType is linked statically (its symbols
  are defined in the executable, no `NEEDED` entry), and fontconfig is
  replaced by the `fontconfig_parser` crate — neither architecture
  references a single `Fc*` symbol, though arm64 still carries a
  `libfontconfig.so.1` `NEEDED`. `README.md` and `docs/guide/index.md`
  list both as required. Measure what a minimal Ubuntu 22.04 actually
  needs to launch, then narrow both files. The set moved on its own when
  the build image changed, so a CI check on the binary's `NEEDED` list
  would keep the docs honest.

- **`strip = true` for the release profile**: shrink release binaries by
  stripping symbols.

- **QuickJS bytecode precompilation**: precompile JS to QuickJS bytecode
  ahead of time instead of parsing source at startup.

- **`.vue` `<style>` blocks don't reach the screen**: scoped/global
  `<style>` blocks in `.vue` SFCs have no effect through the custom
  renderer. A build now fails on one rather than shipping it
  (`ERR_INCA_UNSUPPORTED_STYLE`, raised by `adapter/vite/unsupported.mts`);
  rendering them is Phase 7's work.

- **`dependabot.yml` is missing the `npm`/`cargo`/`github-actions`
  ecosystems**: it currently only auto-updates the devcontainer image/
  features.

- **The window is set once and never follows the app**: `start()`
  (`crates/inca-host/src/main.rs`) reads the size and title out of the
  config and the mounted root, hands them to `WindowOptions`, and nothing
  revisits them. An app that changes its root's `width`/`height`, or wants
  a title that tracks its state, has no way to move the window. GPUI has
  `Window::resize` and `Window::set_window_title`
  (`third_party/zed/crates/gpui/src/window.rs:2622, 2732`); reaching them
  from JS is a binding, and settling which of these an app owns is Phase 9
  item 1.

- **GPUI window options with no `inca.config.ts` surface**: `window_bounds`,
  `titlebar.title`, `is_resizable` and `app_id` are wired; the rest of
  `WindowOptions` is not. Two are already felt. `window_min_size` would
  stop a resizable window shrinking below a fixed-size root and clipping
  it — `resizable: false` only covers the growing side.
  `window_background` picks the colour of whatever the app's root doesn't
  cover, which is what a fixed-size root in a larger window shows; nothing
  sets it, so it is GPUI's default. The remainder (`is_minimizable`,
  `is_movable`, `window_decorations`, `kind`, `display_id`, `focus`,
  `show`, `tabbing_identifier`, `titlebar.appears_transparent`,
  `titlebar.traffic_light_position`) is Phase 9 item 1's scope.

- **A packaged app's output reaches nobody**: `inca dev` relays the host's
  stderr, so an app's `console` output and the host's own reports are
  visible while developing. A double-clicked `.app` writes to a stderr
  nothing reads. A log file beside the app would also give the crash
  reporting in [ROADMAP.md](./ROADMAP.md#known-gaps-not-yet-scheduled)
  somewhere to write. Where it lives per platform, how it rotates, and
  whether an app can opt out are all undecided.

- **A `--print-config` diagnostic**: nothing shows which settings an app is
  actually running under. The host reads `inca.json` beside the entry and
  applies what it can use; a flag that read it and printed the result would
  answer "why is my window that size" without a build. Running it from the
  CLI to validate would make `inca build` need a host binary, which it
  doesn't today — so this is a diagnostic, not a build step.

- **`inca.json`'s shape is declared twice**: `RuntimeConfig` in
  `packages/cli/src/config/loader.mts` and `AppConfig` in
  `crates/inca-host/src/config.rs`. `tests/tests/config_contract.rs` holds
  them to each other, so a member added to one side alone fails. Generating
  one from the other (`ts-rs`, `schemars`) would remove the duplication, at
  the cost of making one language's build depend on the other's — not worth
  it at eight fields. Revisit if the shape grows, or if the dev protocol's
  messages want the same treatment.

- **`icon` names two different things**: `inca.config.ts`'s `icon` is an
  `.icns` path, copied into the macOS `.app` bundle by `inca package`.
  GPUI's `WindowOptions.icon` is an `Arc<RgbaImage>` the X11 backend sets
  on the window, and nothing sets it. A Linux app therefore has no window
  icon at all. Either the config's `icon` feeds both, or the name says
  which one it is.

- **`event.target` is an approximation**: GPUI only gives a container a
  hitbox when something listens on it, so a click on a listener-less child
  can't be traced to that child — `EventDispatcher::dispatch`
  (`crates/inca-bridge/src/dispatch.rs`) sets `target` to the same node as
  `currentTarget`, the node the call is dispatching for. Making it exact
  means giving every container a hitbox while any node in the tree has a
  mouse listener, and reading the innermost one hit_test finds — costing a
  hitbox and a no-op listener call per node per pointer event. The field
  name doesn't need to change for this to land later; only its accuracy
  would improve. Until it does, nothing resembling event delegation
  (a handler relying on which descendant was actually hit) can be written
  against this framework.

- **No capture-phase listener registration**: GPUI's own mouse dispatch
  already runs a capture phase before the bubble phase
  (`Window::dispatch_mouse_event`), but `addEventListener` has no way to
  ask for it — every JS listener is bubble-phase only. Adding it means an
  options parameter on `addEventListener` (`{ capture: true }`, matching
  the DOM) and splitting `EventListeners`' `(node, event)` key into
  `(node, event, capture)`, with `EventDispatcher::listens` and `dispatch`
  each gaining a capture-phase counterpart wired to GPUI's
  `capture_any_mouse_down`/`capture_any_mouse_up` and friends. Vue's
  `@click.capture` has nothing to bind to until this lands.

- **A GPUI-vs-DOM compat layer for `crates/inca-bridge`**: three separate
  gaps now live loose in `dispatch.rs` — the two entries above plus
  `mouseenter` firing on mount for an element already under the pointer
  (GPUI's hover check compares against freshly-initialized state on first
  paint, not against a real pointer move; see `specs/FFI.md`'s "Event
  dispatch" section). `EventDispatcher` is already where this kind of
  translation belongs — it exists to turn GPUI's raw input into what a DOM
  author expects — so grouping these under one `compat` submodule there,
  rather than a separate crate, is the direction: one place to hold this
  session's `held_buttons` tracking alongside a "no real pointer move seen
  yet" flag that would suppress the mount-time `mouseenter`, and later the
  capture-phase and precise-`target` work above.

- **`"focus"`/`"blur"` will need to become `"focusin"`/`"focusout"` once
  focusable nodes can nest**: `crates/inca-bridge/src/focus.rs`'s
  `FocusRegistry` dispatches DOM's non-bubbling `focus`/`blur` today,
  correct only because no focusable node has a focusable descendant yet.
  A focusable container wrapping a focusable child would need the
  bubbling pair instead — checking whether the node whose focus state
  changed is the exact one focused, not just an ancestor of it.

- **No Tab-key focus navigation**: `crates/inca-bridge/src/focus.rs` only
  moves focus on an explicit `focusNode` call or a click landing on a
  focusable, `.id()`-bearing node. GPUI already has `tab_index`/
  `tab_stop`/`window.focus_next(cx)` for this — wiring it means deciding
  what `inca`'s tab-order vocabulary looks like (a style prop? an
  attribute?) and is a separate unit from the focus model itself.
