<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Backlog

Desktop-app-shaped gaps and quality-of-life fixes that fall outside
[ROADMAP.md](./ROADMAP.md)'s phased plan — things noticed while building.
Entries are ordered by ID within a priority, and any of them can be picked up at
any time, independent of the phase currently in progress.

Once a project needs external visibility or outside contribution, this
should move to GitHub Issues instead; for now, a single Markdown file is
enough overhead for a single maintainer plus AI pairing.

Each entry has a stable ID and a metadata line: the units it touches
(bridge, gpui, host, jsenv, core, cli, docs, ci), a size (S, M, L, or a range
such as S–M) and an impact (High, Medium, Low). `Status: needs repro` marks an
entry whose fix needs a manual reproduction first. Entries are grouped by
priority:

- P1: High impact, or picked up first.
- P2: Medium impact.
- P3: Low impact, or blocked on a design decision.

A fixed entry is deleted and its ID is never reused.

## P1

- **B-060 A panic in a packaged app is reported nowhere**
  `Units: host,core · Size: S · Impact: Medium`

  `inca dev` reports a host panic as a `-32603` error. A packaged app has no
  stdout client and its stderr goes nowhere, so a panic there leaves nothing
  behind. It needs a log file or a dialog. In `inca dev` a panic shows twice:
  once from the default hook on stderr, and once as the `-32603` line the CLI
  prints. A host thread blocked writing to a stalled stdout pipe holds the
  hook, though the panic text has already reached stderr. In a production
  build an error an event handler throws is logged to the console only, so the
  destination also decides whether it carries those. It is the first half of
  the crash reporting in
  [ROADMAP.md](./ROADMAP.md#known-gaps-not-yet-scheduled); the destination is
  undecided.

- **B-120 Style keys are read in snake_case only**
  `Units: gpui,docs · Size: M · Impact: High`

  A Vue app writes `:style` keys in camelCase or kebab-case, such as
  `flexDirection` or `'flex-direction'`. The host reads `flex_direction` and
  warns about the other spellings. The vocabulary lives only in the host, so
  one normalising step in `set_style` and `remove_style` can map
  `flexDirection`, `flex-direction` and `flex_direction` to one key, and the
  warning keeps the spelling the app wrote. Normalising once per set leaves
  the render path as it is. `docs/reference/elements.md` lists the CSS
  spelling first.

- **B-121 Style key names and values differ from CSS**
  `Units: gpui,docs · Size: L · Impact: High`

  The host names some keys after its own vocabulary: `text_color` for
  `color`, `background` for `background-color`, `corner_radius` for
  `border-radius`, `text_size` for `font-size`. Values follow the host too:
  `justify_content: "start"` where CSS has `flex-start`, a number of px where
  CSS has `"10px"`. The host can take the CSS names and value forms as
  aliases next to its own and warn with the key as written. B-085
  (percentages, shorthands) and B-117 (wrong-shaped values) belong to the same
  change.

## P2

- **B-007 `.vue` `<style>` blocks don't reach the screen**
  `Units: cli,core,gpui · Size: L · Impact: Medium`

  Scoped/global
  `<style>` blocks in `.vue` SFCs have no effect through the custom
  renderer. A build now fails on one rather than shipping it
  (`ERR_INCA_UNSUPPORTED_STYLE`, raised by `adapter/vite/unsupported.mts`);
  rendering them is Phase 7's work.

- **B-012 The window and the app control each other only at start**
  `Units: bridge,host,core · Size: L · Impact: Medium`

  The host takes the window's size and title from the config and the
  mounted root. Each size axis fits once, when its content size first
  becomes known (`start()` and `maybe_auto_resize_to_content` in
  `crates/inca-host/src/app.rs`). A later change of the root's
  `width`/`height`, or of the title the app wants, leaves the window as it
  is, and the app has no binding to move it. A resize by the user stays in
  the window, and the host sends the app no event for it. The target is a
  window that the app and the user can both control at any time. GPUI has
  `Window::resize` and `Window::set_window_title`
  (`third_party/zed/crates/gpui/src/window.rs:2622, 2732`). Reaching them
  from JS is a binding, and settling which of these an app owns is Phase 9
  item 1.
- **B-014 A packaged app's output reaches nobody**
  `Units: host,cli · Size: L · Impact: Medium`

  `inca dev` relays the host's
  stderr, so an app's `console` output and the host's own reports are
  visible while developing. A double-clicked `.app` writes to a stderr
  nothing reads. A log file beside the app would also give the crash
  reporting in [ROADMAP.md](./ROADMAP.md#known-gaps-not-yet-scheduled)
  somewhere to write. Where it lives per platform, how it rotates, and
  whether an app can opt out are all undecided.

- **B-019 No capture-phase listener registration**
  `Units: bridge,gpui,core · Size: L · Impact: Medium`

  GPUI's own mouse dispatch
  already runs a capture phase before the bubble phase
  (`Window::dispatch_mouse_event`), but `addEventListener` has no way to
  ask for it — every JS listener is bubble-phase only. Adding it means an
  options parameter on `addEventListener` (`{ capture: true }`, matching
  the DOM) and splitting `EventListeners`' `(node, event)` key into
  `(node, event, capture)`, with `EventDispatcher::listens` and `dispatch`
  each gaining a capture-phase counterpart wired to GPUI's
  `capture_any_mouse_down`/`capture_any_mouse_up` and friends. Vue's
  `@click.capture` has nothing to bind to until this lands, so
  `patchProp.mts` strips the suffix and binds an ordinary bubble-phase
  listener instead. `.passive` degrades the same way, for the same
  reason — no passive-listener notion anywhere in the bridge yet.
  `.once` doesn't need native support and is implemented for real:
  the listener unbinds itself after firing.

- **B-020 A GPUI-vs-DOM compat layer for `crates/inca-bridge`**
  `Units: bridge · Size: M · Impact: Medium`

  `EventDispatcher` turns GPUI's raw input into DOM events, and two gaps sit
  loose in `dispatch.rs`: the `held_buttons` update that runs only for a
  dispatched listener (B-045) and the capture phase (B-019). Direction: one
  `compat` submodule in the bridge crate holding the `held_buttons` tracking
  and later the capture-phase work.

- **B-028 A template `ref` still resolves to a plain data object for anything beyond `.focus()`/`.blur()`**
  `Units: core · Size: M · Impact: Medium`

  `IncaElement`
  (`packages/core/src/vue/nodeOps.mts`) has those two methods; the same
  gap likely applies to any other DOM-element method or property a
  `.vue` app would otherwise reach for on a template `ref`.

- **B-030 A style-only change to an already-mounted node doesn't reach the screen under `--experimental-hmr`, though the same edit works via a full reload**
  `Units: gpui,bridge,cli · Size: M · Impact: Medium · Status: needs repro`

  Reproduced by hand against `examples/click_counter`,
  editing only a `:style` value (e.g. `border_color`) with the script and
  template structure otherwise unchanged. Confirmed at every layer up to
  and including the native tree: a diagnostic harness driving the real
  Vue HMR update path (`@vitejs/plugin-vue`'s `rerender`, which patches
  the existing component instance in place rather than remounting it)
  showed the correct new value reaching `__inca_native__.setStyle` for
  the right node, both with a minimal fixture and with `click_counter`'s
  actual source. Editing something that changes the mounted tree's shape
  instead — adding, removing, or changing text content — updates the
  screen correctly and immediately, as does a plain (non-HMR) full
  reload's rebuilt tree. What's different about the failing case: a full
  reload rebuilds the whole session and tree, so its elements are mostly
  new to `gpui`. B-069: node ids restart on a full reload, so `gpui` can
  see a reused id after a full reload. `--experimental-hmr`'s in-place `rerender` is the
  first path in this project that mutates a style property on a node
  `gpui` has already seen, in the same long-lived window, with the same
  element id, and nothing else about that node changing. That points at
  `gpui`'s own element/paint reuse for a stable element id not
  accounting for a style-only change with no other difference. Not
  reproduced outside this pattern — a full reload's own repaint after a
  style edit already works, and always has. A removed style key is
  another style-only change to an already-seen node, so it is probably
  affected the same way; that is not confirmed. Not reproduced on Intel
  macOS.

- **B-031 `inca dev --experimental-hmr` never recovers from a source file that was already broken when the session started**
  `Units: cli · Size: M · Impact: Medium`

  A live edit that
  introduces a compile error and is later fixed correctly triggers a
  normal update; starting the session against an already-broken file
  doesn't — fixing it afterward produces no update, only restarting
  `inca dev` recovers. Reproduced by hand: launch against a `.vue` file
  with a syntax error already present, then correct it while the session
  keeps running. Cause: the very first `import()` never completes, so
  Vite's dev server never adds the file to its module graph, leaving it
  nothing to invalidate once the file changes. Likely fix: retry that
  `import()` directly once a later file change is detected, instead of
  relying on Vite's own graph-based invalidation. The same broken
  startup also duplicates its own failure report, a separate, more
  general gap B-032 covers.

- **B-036 A script edit's new value doesn't reach a real click, once it reverts to a value already used earlier in the same session**
  `Units: gpui,cli · Size: M · Impact: Medium · Status: needs repro`

  reproduced by hand against `examples/click_counter`, repeatedly
  changing `onClick`'s `clicks.value += 1` to `+= 2`, then `+= 3` — each
  takes effect correctly on the next click. Changing it back to `+= 1`
  doesn't: clicks keep incrementing by whatever amount was last
  genuinely applied. Ruled out at the Node/Vite layer: a diagnostic
  driving the exact same edit sequence through a synthetic dev-protocol
  stimulus, bypassing gpui's own click dispatch entirely, applies every
  value correctly, reverts included. Vite always serves fresh, correct
  code, confirmed by inspecting the served module's own source each
  time. The difference from that diagnostic is a real click, dispatched
  through gpui's own event system, pointing at the same element/paint
  reuse already suspected in B-030, this time for an event
  listener closure rather than a style property, on Vue's own `reload`
  path rather than `rerender`. Not confirmed at the native layer the way
  B-030 was. Reproduced on Intel macOS, where B-030 is not.

- **B-045 `buttons` stays set after a release nobody listens to**
  `Units: bridge · Size: M · Impact: Medium`

  `update_held_buttons` (`crates/inca-bridge/src/dispatch.rs`) only runs
  when a listener is dispatched. A node that listens for `mousedown` and
  `mousemove` but not `mouseup` never clears the pressed bit, so later
  `mousemove` events report the button as held. Fix: track buttons from the
  raw window event, in the pointer tracker of `inca-gpui` or the `compat`
  submodule of B-020.

- **B-048 A multi-root `App` (or a top-level comment) breaks window sizing and `gap`**
  `Units: host,gpui · Size: M · Impact: Medium`

  Vue marks a Fragment's edges with empty text nodes.
  `content_window_size` (`crates/inca-host/src/app.rs`) reads the root's
  first child, so it reads an empty text node, falls back to 800×600, and
  never auto-resizes. Those text nodes are also real flex children, so
  `gap` adds space around every `v-for`.

- **B-050 `FAILURES.md` promises a window-size check that doesn't exist**
  `Units: cli,docs · Size: S–M · Impact: Medium`

  `handbook/FAILURES.md` says a `width` of `-100` "is refused, and the message
  says which value and which file". The CLI checks nothing, and the host's
  `usable()` quietly falls back to the default. One wrongly typed field
  (`width: "800"`) makes the host ignore the whole `inca.json` (name,
  identifier and title too), with only a stderr line. Either validate in
  the CLI or change what `FAILURES.md` claims.

- **B-053 `focus`/`blur` listeners that call `focusNode` may not take effect**
  `Units: bridge,host · Size: S–M · Impact: Medium`

  `focus` and `blur` listeners run inside `render()`, where GPUI ignores
  `window.refresh()` (`third_party/zed/crates/gpui/src/window.rs:2178`), so
  the request may wait for an unrelated redraw. Unconfirmed.

- **B-059 No engine limits or interrupt handler**
  `Units: jsenv,host · Size: M · Impact: Medium`

  `EngineBuilder`
  (`crates/inca-jsenv/src/engine.rs`) sets no memory limit, stack size, or
  interrupt handler, so a runaway app can't be stopped. rquickjs offers
  `Runtime::set_memory_limit`/`set_max_stack_size`. Defaults and how the
  host reports a timeout are undecided.

- **B-068 `click` fires before `mouseup`**
  `Units: gpui · Size: M · Impact: Medium`

  `click` and `auxclick` fire before `mouseup` on the same node, and
  `dblclick` follows the second `click` there. `stopPropagation()` in a
  `click` handler also stops ancestors' `mouseup` and `dblclick`. The browser
  order is `mouseup`, `click` or `auxclick`, then `dblclick`.

- **B-087 Ctrl+C in `inca dev` leaves the terminal echoing arrow keys**
  `Units: cli,host · Size: S–M · Impact: Medium · Status: needs repro`

  After `inca dev` is stopped with Ctrl+C, the shell echoes arrow keys as
  `^[[A` instead of recalling the previous command, until the terminal is
  reset. The cause is not known. It may be the terminal mode left behind by
  the host or by the CLI process, or a shell that was not given the terminal
  back. Reproduce on each platform with the CLI run directly and through
  `pnpm run`, and compare the state with `stty -a` before and after.

- **B-088 Stack frames point into generated code**
  `Units: cli,host,jsenv · Size: L · Impact: Medium`

  A stack printed by `console` or reported by the host does not lead back to
  the source. With the HMR runtime, a frame reads `at name (<input>:line:column)`.
  The runtime builds each module with an `AsyncFunction`, and QuickJS gives
  every function made by the `Function` constructors the file name `<input>`;
  it has no `sourceURL` support. The line numbers are lines of the Vite
  transform of the module. Without HMR the whole app is one bundle, so a
  frame names `bundle.js` and a line of the generated code, since the build
  emits no source map. The dev runtime also disables Vite's source map
  support, which expects V8 call sites. A fix has two parts. The HMR runtime
  needs a host-provided function that compiles a module's code under its
  module id. Then the build and the dev transform emit source maps, and the
  host remaps each frame to the `.vue` or `.ts` file, line and column. This
  also covers errors the host reports to `inca dev`.

- **B-097 `console` prints some classes and built-ins unlike Node**
  `Units: jsenv · Size: M · Impact: Medium`

  Properties keyed by a `Symbol` and extra properties on an array or an
  `Error` are dropped. A `Promise`, a typed array, a `WeakMap` and a boxed
  primitive print as a plain object: `{}` for most, `{ '0': 0, '1': 0,
  '2': 0 }` for `new Uint8Array(3)`, where Node prints `Promise { 1 }` and
  `Uint8Array(3) [ 0, 0, 0 ]`. A nested string, and an object key that is
  not an identifier, is always single-quoted, where Node picks the quote
  that avoids escaping.

- **B-122 Colours take fewer notations than CSS**
  `Units: gpui,docs · Size: M–L · Impact: Medium`

  A colour is a `0xRRGGBB` number or a `#rgb` or `#rrggbb` string. CSS adds,
  from small to large: `#rgba` and `#rrggbbaa`, `transparent` and
  `currentcolor`, `rgb()`, `rgba()`, `hsl()` and `hsla()` in the comma and
  the space syntax with an alpha, the 148 colour names, `hwb()`, `lab()`,
  `lch()`, `oklab()`, `oklch()`, `color()`, `color-mix()`, relative colours,
  `light-dark()` and system colours. `var(--x)` waits for CSS variables.
  gpui colours carry an alpha, so the parser sets the limit. A parsing crate
  such as `csscolorparser` covers part of the list, and its range needs a
  check before use. Only `background`, `border_color` and `text_color` take a
  colour. `outline-color`, per-side border colours, `box-shadow` and
  `text-decoration-color` come with the properties they belong to.

- **B-124 `inca` has no command that reports the host's settings**
  `Units: cli · Size: M · Impact: Medium`

  The host prints the settings it applies with `--print-config`, and only
  the host binary takes that flag. `inca config` could resolve the app's
  config as `inca dev` does, write it with an empty entry under
  `node_modules/.inca/config/`, run the host with `--print-config` and print
  the JSON with the host's exit code. The host comes from `resolveHostBin`,
  so `INCA_HOST_BIN` and the cache of a compressed host both apply. A
  follow-up, `inca info`, prints the Node version, the platform, the host
  package, the host binary's path and the `INCA_*` overrides.

- **B-126 A missing `@vue/runtime-dom` fails the build with the bundler's own error**
  `Units: cli,core,docs · Size: S–M · Impact: Medium`

  Compiled templates import `incajs/vue/runtime`, which re-exports from
  `@vue/runtime-dom`, an optional peer of `incajs`. When the app lacks the
  package, `inca build` and `inca dev` fail with the bundler's unresolved
  import message. It names the package and gives no install hint. A check in
  the Vite adapter could print the install command, as the adapter does for
  unsupported style blocks. No test installs the packed `incajs` into a
  scratch app and resolves `incajs/vue/runtime` through the published
  `exports`. The UI tests cover the workspace link.

## P3

- **B-001 Vue template type-checking**
  `Units: core · Size: M · Impact: Low`

  `vue-tsc` mechanically works (verified
  against a vanilla `create-vue` scaffold), but it wouldn't catch anything
  in this repo's `.vue` files today. There's no `JSX.IntrinsicElements`/
  `GlobalComponents` typing wiring `packages/core`'s `StyleProps`
  (`packages/core/src/types.mts`) into Vue's template type system, so a
  `:style` binding or a `v-for` has nothing to check against. Declaring
  that typing is the prerequisite; only after that does adding `vue-tsc`
  (plus the full `vue` package, `@vue/tsconfig`) actually buy anything.

- **B-002 Linux runtime dependencies in `inca package`**
  `Units: cli · Size: L · Impact: Low`

  Measured on an
  `aarch64` release build — 8 direct `NEEDED` libraries, desktop-typical
  except `libxkbcommon-x11`. Vulkan and Wayland are `dlopen`ed, and a
  Vulkan ICD is GPU-specific, so neither can be bundled meaningfully. The
  binary carries no `RPATH`/`RUNPATH`, so bundling anything means adding
  an `$ORIGIN` rpath (as `third_party/zed/script/bundle-linux` does) or a
  launcher first. `FREETYPE2_NO_PKG_CONFIG=1` drops `libfreetype` by
  building it statically; `RUST_FONTCONFIG_DLOPEN=1` only defers the
  failure to runtime. Whether this is worth doing at all is the question.

- **B-003 The documented Linux dependencies are wider than the binary's**
  `Units: docs,ci · Size: S–M · Impact: Low`

  In
  v0.0.3's published binaries FreeType is linked statically (its symbols
  are defined in the executable, no `NEEDED` entry), and fontconfig is
  replaced by the `fontconfig_parser` crate — neither architecture
  references a single `Fc*` symbol, though arm64 still carries a
  `libfontconfig.so.1` `NEEDED`. `README.md` and `docs/guide/index.md`
  list both as required. Measure what a minimal Ubuntu 22.04 actually
  needs to launch, then narrow both files. The set moved on its own when
  the build image changed, so a CI check on the binary's `NEEDED` list
  would keep the docs honest.

- **B-005 The dev protocol always ships inside `inca-host`**
  `Units: host,cli · Size: L · Impact: Low`

  `crates/inca-host/src/dev.rs`
  (stdin/stdout JSON-RPC, the stdout writer thread, `reload`) compiles into
  every `inca-host` binary, including the ones `inca package` distributes,
  even though a packaged app never passes `--dev` and never runs any of it.
  Worth a way to drop it from a release build — a Cargo feature gating
  `mod dev;` and the CLI's `--dev` flag, built once for `inca dev`'s own use
  and once (without the feature) for packaging — once it's worth the two
  build configurations that implies. The code is about 0.14% of the binary,
  so the gain is a smaller surface in a distributed app. Decide it together
  with how a packaged app finds and trusts its bundle.

- **B-006 QuickJS bytecode precompilation**
  `Units: jsenv,host,cli · Size: L · Impact: Low`

  Precompile JS to QuickJS bytecode
  ahead of time instead of parsing source at startup.

- **B-008 `dependabot.yml` is missing the `npm`/`cargo`/`github-actions` ecosystems**
  `Units: ci · Size: S · Impact: Low`

  It currently only auto-updates the devcontainer image/
  features.

- **B-013 GPUI window options with no `inca.config.ts` surface**
  `Units: host,cli · Size: M–L · Impact: Low`

  `window_bounds`,
  `titlebar.title`, `is_resizable`, `window_min_size` and `app_id` are
  wired; the rest of `WindowOptions` is not. `window_background` is felt:
  it is a `WindowBackgroundAppearance` (opaque, transparent, blurred), not
  a colour, so it can't fill what a fixed-size root in a larger window
  leaves uncovered. A background colour needs a full-size wrapper node in
  `HostedApp::render`, plus a matching `RuntimeConfig` field and an update
  to `tests/tests/config_contract.rs`. The remainder (`is_minimizable`,
  `is_movable`, `window_decorations`, `kind`, `display_id`, `focus`,
  `show`, `tabbing_identifier`, `titlebar.appears_transparent`,
  `titlebar.traffic_light_position`) is Phase 9 item 1's scope.

- **B-016 `inca.json`'s shape is declared twice**
  `Units: cli,host · Size: M · Impact: Low`

  `RuntimeConfig` in
  `packages/cli/src/config/loader.mts` and `AppConfig` in
  `crates/inca-host/src/config.rs`. `tests/tests/config_contract.rs` holds
  them to each other, so a member added to one side alone fails. Generating
  one from the other (`ts-rs`, `schemars`) would remove the duplication, at
  the cost of making one language's build depend on the other's — not worth
  it at eight fields. Revisit if the shape grows, or if the dev protocol's
  messages want the same treatment.

- **B-017 `icon` names two different things**
  `Units: host,cli · Size: M · Impact: Low`

  `inca.config.ts`'s `icon` is an
  `.icns` path, copied into the macOS `.app` bundle by `inca package`.
  GPUI's `WindowOptions.icon` is an `Arc<RgbaImage>` the X11 backend sets
  on the window, and nothing sets it. A Linux app therefore has no window
  icon at all. Either the config's `icon` feeds both, or the name says
  which one it is.

- **B-026 More `MouseEvent`/`KeyboardEvent` fields could reach JS**
  `Units: gpui,bridge · Size: M–L · Impact: Low`

  `crates/inca-gpui/src/event_sink.rs`'s `MousePayload`/`KeyPayload`
  carry a deliberate subset of each DOM type's own fields. What's between
  each missing one and landing differs:
  - Computable today, but not a small change — no `gpui` change needed.
    `offsetX`/`offsetY` are measured from the event's `target`. The
    target's bounds are computed once, for hitbox insertion, and need to be
    kept where `dispatch` can reach them, since `gpui` has no production API
    to look a node's bounds up by id at event time.
  - `screenX`/`screenY` are `Window::bounds`' origin plus the client
    position. Wayland reports no window position, so they equal
    `clientX`/`clientY` there, and X11 may report an origin that leaves out
    the window decorations. The fix direction is a platform query in `gpui` for the
    client area's screen position.
  - `isComposing` needs the text-editing/IME unit, which hasn't started.
  - Blocked on `gpui` itself: `code` and `location` — `Keystroke`
    (`third_party/zed/crates/gpui/src/platform/keystroke.rs`) carries
    only `{ modifiers, key, key_char }`, with no layout-independent
    scancode and no left/right or numpad distinction to recover either
    from. `location` is 0 for every key and `code` is absent. The fix
    direction is a `gpui` platform change that reports the scancode and
    the key location, then `code` and `location` read them.
  - `getModifierState` supports Control, Shift, Alt, Meta and Accel.
    `gpui`'s `Modifiers` holds four flags (plus `function`), so the lock
    keys (`CapsLock`, `NumLock`, `ScrollLock`) and `AltGraph`, `Fn`,
    `Hyper`, `Super` and `Symbol` return false. The fix direction is a
    platform query added to `gpui` for the lock state.
  - `key` for a shifted letter is upper case and for Shift+1 with no
    `key_char` it is `"1"`. The fix direction is a keyboard layout table in
    `inca-gpui`, or the layout's shifted character reported by `gpui`.

- **B-145 `contextmenu` fires on the press on every platform**
  `Units: gpui,bridge · Size: S · Impact: Low`

  `contextmenu` fires after `mousedown` of the secondary button, which is the
  macOS and Linux timing. Windows fires it after `mouseup`. Fix: when a
  Windows host ships, the root tracker in `inca-gpui` dispatches it from the
  secondary button's release on that platform.

- **B-146 Pointer-field values differ between engines**
  `Units: bridge,docs · Size: S · Impact: Low`

  `click`, `auxclick` and `contextmenu` report `pointerId` 1, `isPrimary` true
  and, for `contextmenu`, `detail` 0. Firefox reports `pointerId` 0 and
  `contextmenu` `detail` 1, and Chromium reports `isPrimary` false. Fix:
  revisit the values when the `pointer*` events (B-148) land.

- **B-148 The `pointer*` events are unsupported**
  `Units: gpui,bridge,docs · Size: L · Impact: Medium`

  `pointerdown`, `pointerup`, `pointermove`, `pointerover`, `pointerout`,
  `pointerenter`, `pointerleave`, `pointercancel`, `gotpointercapture` and
  `lostpointercapture` have no effect. Fix: dispatch them from the same mouse input
  with the `PointerEvent` fields `click` carries, then add pointer capture.

- **B-027 No input-synthesis/state-introspection channel on the dev protocol**
  `Units: host,cli · Size: L · Impact: Low`

  `tests/tests/hmr-quickjs-state.test.mts` drives its fixture with a
  `tick` dev notification standing in for a pointer click, because
  there's no way to synthesize one against a real running `inca-host`
  process from outside. A generalized version of that workaround — a dev
  protocol method for synthesizing input and reading back app state —
  would be the seed of future automated GUI testing (a Playwright-style
  tool driving the app), not just this one test's stimulus.

- **B-029 `inca dev --experimental-hmr` opens its window at the wrong size on first launch, then resizes**
  `Units: host,jsenv · Size: L · Impact: Low`

  A plain `inca dev` opens already sized to
  the app's own declared content (no visible gap). Under
  `--experimental-hmr`, the window instead opens at the app's configured
  size or a default, with the app's real content appearing moments later
  once mounting finishes, then a one-time resize snaps it to the correct
  size — visibly, a large window with black margins that shrinks after a
  beat. Cause: in HMR mode, the entry module `inca-host` evaluates doesn't mount
  the app itself. It hands off to a JS module runner that fetches and
  evaluates the real app code over a round trip to the Node-side dev
  server. `inca-host` doesn't wait for that round trip before opening the
  window: waiting needs the process's stdin reader, which answers that
  round trip, to already be running, and today it only starts once the
  window has opened. A real fix starts that stdin reader before the window
  opens, and keeps servicing incoming replies while module evaluation is
  still in progress, so the app's real content size is known before the
  window is created. The engine's module-evaluation entry point
  (`Engine::eval_module`) doesn't support that today: it drives a module's
  own top-level `Promise` to completion synchronously, and reports an
  error if the `Promise` can't settle on its own. There's no hook for
  external I/O to arrive and resolve it mid-call, so making the entry
  await its own mount would just fail immediately instead of waiting.
  Fixing this changes `inca-host`'s own startup sequencing, not just this
  feature — carried forward rather than attempted alongside
  the rest of experimental HMR.

- **B-032 The HMR bootstrap's rejection handler doesn't distinguish who's responsible for reporting a failure**
  `Units: cli · Size: S · Impact: Low`

  A Vite/bundler-caused failure
  is Node's own event, and Node already reports it (`hmr.mts`'s
  `onError`). A genuine error in the running app's own code, unrelated
  to Vite, has no other reporter, and belongs to the app to report. The
  HMR bootstrap script (`writeHmrEntry` in `hmr.mts`) doesn't make this
  distinction: every rejected `import()` goes to `console.error(err)`
  inside the running app, regardless of which side actually caused it.
  A Vite-caused failure at startup therefore prints twice — once from
  Node, once raw and unstyled from the app (see B-031). Fix:
  have the bootstrap recognize a Vite-shaped rejection and skip its own
  report for that one case, while still reporting anything else itself.

- **B-033 `[inca] reload`/`[inca] update`'s timing isn't the useful number**
  `Units: cli,host · Size: M · Impact: Low`

  both only measure how long the bundler took to prepare an update, not
  how long until the app actually applies it and the screen reflects
  it. Getting the real number needs the host to confirm back to Node
  once an update is actually applied, which no dev-protocol message
  does today.

- **B-035 A `full-reload` HMR payload skips the ready/pendingReload guard, and reports nothing**
  `Units: cli · Size: S · Impact: Low`

  `dev.mts`'s `reload: () => void reloadHost()` (passed
  to `bundler.hmr`) calls straight through to `reloadHost()` regardless of
  whether the host has finished its first load — the non-HMR `onBuild`
  path guards this with `pendingReload`, but a `full-reload` payload has no
  equivalent. It also logs nothing on success, unlike the "reload ... (Nms)"
  line `onBuild` prints, so a full reload under `--experimental-hmr` is
  invisible to the user.

- **B-037 Dev relay hardening (`__inca_dev__`)**
  `Units: bridge,host · Size: S–M · Impact: Low`

  Three related gaps in
  `crates/inca-bridge/src/dev.rs`. `install_dev`'s `send` accepts any method
  name, including the host's own reserved ones (`ready`, `appError`), so a
  running app can spoof a host notification. `call_dev_receive` discards
  whatever `.ok()?` swallows when reading `__inca_dev__`/`receive` off
  globals, rather than capturing it as an `EngineError`. `receive` is
  invoked via a bare `Function::call`, leaving `this` unbound instead of
  bound to `__inca_dev__` — `handbook/PROTOCOL.md`'s own
  `__inca_dev__.receive?.(...)` implies a method call. Also,
  `PROTOCOL_VERSION` (`crates/inca-host/src/protocol.rs`) is still `0`
  despite this new relay surface landing; bump it or write down that it's
  a backstop that doesn't move on every feature addition.

- **B-038 The stdout writer channel is unbounded**
  `Units: host · Size: M · Impact: Low`

  `StdoutWriter::spawn`
  (`crates/inca-host/src/dev.rs`) backs its writer thread with
  `async_channel::unbounded`, so a client that stops reading stdout never
  blocks the host — it grows the host's memory without limit instead.
  Fixing the behavior itself means a bounded channel with real backpressure.

- **B-069 Node ids restart on every full reload while the window survives**
  `Units: gpui,host · Size: M · Impact: Low`

  Nodes are keyed to gpui by a node id that restarts at 0 on every full reload
  while the window survives. Per-element state such as a pending mouse-down or
  hover can carry over to a different node. A scroll offset can likewise carry
  over to a new scrolling container that gets the same id. Found by reading;
  not reproduced.

- **B-071 Last protocol lines can be lost when quitting on macOS**
  `Units: host · Size: S · Impact: Low`

  On macOS quitting ends the process before the stdout writer is closed, so the
  last queued protocol lines can be lost. Found by reading; not reproduced.

- **B-074 Dev and production differ in `TextDecoder` and `URL`**
  `Units: cli · Size: S · Impact: Low`

  With `--experimental-hmr` the runtime installs a Latin-1 `TextDecoder` and a
  minimal `URL` on the app's `globalThis`. A production build has neither, so
  dev and production differ.

- **B-075 A failed host start in inca dev is handled differently per mode, and the checks overlap**
  `Units: cli · Size: S–M · Impact: Low`

  `dev()` stops right away when no host binary resolves or the path is not a
  file. `startHost` handles every other start failure. Without HMR it prints
  "failed to start inca-host", keeps running and retries on the next rebuild.
  With HMR it stops.

  A file that exists but is not executable passes `assertHostBin` and takes
  the mode-dependent path above.

  `dev()` and `HostClient.start` each call `assertHostBin` on the same path.

  A possible fix: every start failure ends `inca dev` in both modes, which
  removes the retry path. Keep one check, and decide whether it should verify
  the execute permission.

- **B-076 Bridge tree errors differ from the DOM's**
  `Units: bridge · Size: S · Impact: Low`

  A cycle throws a `HierarchyRequestError` `DOMException` in the DOM, but a
  `TypeError` from `appendChild`/`insertBefore`, so code checking
  `error.name` sees a different value. Separately, the DOM allows moving a
  parentless root under a node that is not its descendant. `appendChild`
  and `insertBefore` refuse the root as the child, and `destroyNode`
  refuses to destroy it, because the root belongs to the host.

- **B-077 An element nested in `<text>` renders only its text**
  `Units: gpui · Size: M · Impact: Low`

  A `<text>` element renders its `value` and its descendants' text as one
  string. An element nested in it contributes only its text and loses its
  style and listeners. The DOM renders inline children.

- **B-078 The `scroll` event is not supported**
  `Units: gpui,bridge,core · Size: M · Impact: Low`

  The DOM fires `scroll` when an element's offset changes. A container with
  `overflow` set to `scroll` moves its content but nothing reaches JS.

- **B-079 Scrollbars are not supported**
  `Units: gpui · Size: M · Impact: Low`

  The DOM shows a scrollbar for `overflow: scroll`. Here `gpui`'s scrollbar
  width stays 0, so the wheel is the only way to scroll.

- **B-080 Some `overflow` values are not accepted**
  `Units: gpui · Size: S · Impact: Low`

  `overflow: clip` clips like `hidden` without creating a scroll container.
  `overflow` accepts `visible`, `hidden`, `scroll` and `auto` only, and
  `clip` is ignored like any other unknown value. The two-value form such as
  `hidden scroll` is ignored the same way, since only a single string is read.

- **B-081 `stopPropagation()` on events other than `wheel` cuts `gpui`'s own bubble**
  `Units: bridge · Size: M · Impact: Low`

  `stopPropagation()` on any event but `wheel` also stops `gpui`'s own
  ancestor listeners. The DOM stops only the ancestors' JS listeners.
  In a descendant's `mouseenter`/`mouseleave` callback it can also stop an
  ancestor's hover re-check, since `gpui` runs hover tracking in the same
  bubble-phase `mousemove` dispatch.

- **B-082 Wheel scrolling keeps `gpui`'s axis rules and has no gesture latching**
  `Units: gpui · Size: M · Impact: Low`

  A vertical wheel scrolls a container that scrolls only on x, and only the
  dominant axis of a diagonal delta moves. Both are `gpui`'s rules. The DOM
  latches a continuing gesture to the container it started on. Here a
  gesture moves to the parent as soon as the inner container reaches its
  limit.

- **B-083 Wheel rollback depends on `gpui` applying the scroll step in the event**
  `Units: gpui,bridge · Size: M · Impact: Low`

  A scroll container's `preventDefault()` and nested chaining undo `gpui`'s
  scroll step after the fact. This needs `gpui` to apply the step
  synchronously in the event. A `gpui` with smooth or deferred scrolling
  breaks it. `a_cancelled_scroll_leaves_the_bounds_unmoved` in
  `crates/inca-bridge/tests/scroll_dispatch.rs` checks the bounds after a
  cancel. Each scroll container adds one zero-size element, and the handles
  of removed nodes live until the next full reload.

- **B-084 Wheel scrolling differs from the DOM at the edges**
  `Units: gpui,bridge · Size: M · Impact: Low`

  There is no `overscroll-behavior` and no rubber-banding. `@wheel.passive`
  is accepted and binds as an ordinary listener (see B-019), so
  `preventDefault()` in such a listener still cancels the scroll. Line deltas
  are scaled by `gpui`'s line height, which differs from the browser's.

- **B-085 Some box style values are not accepted**
  `Units: gpui · Size: M · Impact: Low`

  `width`, `height`, `min_*` and `max_*` take px numbers (or `"auto"`) only,
  and `padding` and `margin` take px numbers (`margin` also `"auto"`), so
  percentages are ignored, and `flex_basis` is not a key. `padding` and
  `margin` read one value per key, so the CSS shorthand `"1 2 3 4"` is
  ignored. The length keys `width`, `height`, `min_*` and `max_*` accept
  `NaN` and infinity.

- **B-086 Some `console` methods only approximate the standard**
  `Units: jsenv · Size: M · Impact: Low`

  A format string supports `%s`, `%d`, `%i`, `%f`, `%o`, `%O`, `%c` and `%%`.
  `%o` follows Node with `showProxy` off. `%c` takes its argument and applies
  no styling, and the Console Standard leaves `%c` undefined. Fix: map the CSS
  properties `color`, `background-color`, `font-weight`, `font-style` and
  `text-decoration` to terminal styles. A conversion that throws prints the
  value as `dir` does, where the standard throws. A `Symbol` or a label whose
  `toString` throws becomes the default label where the standard throws.
  `time`, `timeLog` and `timeEnd` always print milliseconds. `dir` ignores its
  `options`. `dirxml` formats like `log`. `clear` closes the open groups and
  leaves the terminal as it is. `groupCollapsed` prints its lines like
  `group`, since a terminal cannot fold them. `table` ignores a `columns`
  argument that is not an array, and drops properties keyed by a `Symbol` and
  a `Symbol` column name. `trace` reads its stack through the global `Error`,
  which a script can replace.

  Printed values differ from the standard in these ways:

  - A string nested in an array or object, and an object key that is not an
    identifier, is quoted without escaping backslashes or control
    characters, so it can break a line.
  - An object or collection that refers back to itself prints `[Object]`,
    `[Array]`, `[Map]` or `[Set]` at the depth limit, with no circular
    marker. An `Error`'s `cause` is not printed.
  - A `Map`, `Set`, `Date` or `RegExp` prints without its own properties and
    without its subclass name, where the standard prints both, as in
    `Map(0) { foo: 1 }`. An object that only inherits from `Map.prototype`
    prints as `{}`. A `Proxy` around any of the four prints as its target.
  - A long object, array, `Map` or `Set` is never wrapped across lines,
    where the standard wraps long output.
  - A `RegExp` has one colour, where the standard highlights its parts.
  - `%s`, `%d`, `%i` and `%f` convert an array that holds a `Proxy` through
    the array's own string form, which runs the `Proxy`'s traps.

- **B-092 Event props differ from Vue's in name and value handling**
  `Units: core · Size: S · Impact: Low`

  `onFooBar` binds `foobar` where Vue binds `foo-bar`. The `on:foo` form is
  not recognized, and any key whose third character is not a lower-case
  letter is an event in Vue but only `on` plus an upper-case letter is one
  here. Vue skips `onUpdate:x` model listeners, where this binds an event
  named `update:x`. A prop named `onOnce` or `onCapture` is read as a
  modifier, where Vue binds the event `once` or `capture`. A non-function
  value unbinds silently, where Vue warns and keeps a no-op listener. An
  empty array unbinds, where Vue keeps the listener.

- **B-093 A control character in a `productName` writes an invalid `Info.plist`**
  `Units: cli · Size: S · Impact: Low`

  `encodePlist` escapes `&`, `<` and `>` but passes every other character
  through. A `productName` or `identifier` holding a control character such
  as `U+0001` produces a document that XML 1.0 forbids, which `plutil -lint`
  rejects. The config check could refuse such a name, or the encoder could
  drop the character; neither is decided.

- **B-094 Intel macOS hosted runners are being retired**
  `Units: ci · Size: M · Impact: Low`

  GitHub has announced that it will stop supporting the Intel architecture on
  macOS hosted runners after the macOS 15 image retires in autumn 2027.
  `macos-26-intel` exists and no newer notice gives its retirement date.
  `ci.yml` and `cd.yml` build the x64 macOS host on that runner, so its end
  date limits how long the host can be built natively. A fallback is to
  cross-compile `x86_64-apple-darwin` on the arm64 runner and run a smoke
  test under Rosetta 2, which does not exercise Metal on Intel hardware.

- **B-095 The platform lists are kept in sync by hand**
  `Units: ci,cli,docs · Size: S · Impact: Low`

  The set of published host platforms is written in `npm/*/package.json`, the
  CLI's `optionalDependencies`, `hostBin.mts`, `cd.yml` (the matrix, the
  manifest list in `verify`, and two `for pkg in` loops), the README and guide
  tables, and the Makefile's `LICENSE_PACKAGES`. A platform missing from the
  `Place host binaries` loop publishes a package with no binary, and an npm
  publish cannot be undone. A test could read all of them and compare against
  the non-private `npm/*` manifests, since a platform is published when its
  manifest is not private. A `publish` step could check that every non-private
  host package has an executable `bin/inca-host` before `pnpm publish`. This
  is needed before the Windows hosts are added.

- **B-096 `missing_docs` is not enforced**
  `Units: gpui,bridge,ci · Size: M · Impact: Low`

  `inca-bridge` lacks docs on 4 public items and `inca-gpui` on 59, as
  `RUSTDOCFLAGS="-W missing-docs" cargo doc -p <crate> --no-deps` counts
  them. `inca-jsenv` and `inca-host` have none. Setting
  `missing_docs = "warn"` under `[workspace.lints.rust]` applies to every
  crate that inherits the table. It fails CI, which runs clippy with
  `-D warnings`, until the gaps are filled. A crate cannot opt out of one
  inherited lint, so a crate that is not ready needs
  `#![allow(missing_docs)]` in its source.

- **B-098 The app cannot hook a reload or a shutdown**
  `Units: bridge,core,host · Size: M · Impact: Low`

  Nothing tells an app that a reload is about to discard its session or
  that the host is shutting down. The intended design adds no binding. The
  host dispatches a named event on `rootNodeId()` through the existing
  `addEventListener` path. The hook is observe-only: when a reload's hook
  would fire, the new bundle has already loaded, so there is nothing left to
  veto. A handler can await only microtasks that have already settled,
  because the job queue drains once after the hook and QuickJS has no timer
  or I/O to resume it. `packages/core` wraps the event names in helpers, so
  the strings never become app-facing API.

- **B-101 `rust-version` has no floor check of its own**
  `Units: ci · Size: S · Impact: Low`

  `rust-version` equals the pinned toolchain, so it states no real floor.
  That holds while the crates have no consumers outside this repository.
  Once app crates compile against them (roadmap Phase 13), `rust-version`
  drops below the pin and needs its own CI job. The job installs the floor
  toolchain and runs `cargo +<msrv> check`, where `+<msrv>` overrides
  `rust-toolchain.toml`. It is a second toolchain, so it adds a second full
  gpui build under its own `Swatinem/rust-cache` key.

- **B-102 The HMR state e2e test timed out once in CI**
  `Units: ci,cli · Size: S · Impact: Low`

  `tests/tests/hmr-quickjs-state.test.mts` timed out once in CI at the edit
  that fixes a broken script: the second mount did not show up within the
  30 s `WAIT_TIMEOUT_MS`. The same job passed on a re-run. The path is the
  recovery from a broken live edit, which works in manual use. The test's
  own comments note a race between the `file-changed` event and the
  module's re-evaluation. A longer timeout would hide the cause. Logging the
  host's stderr and the HMR events at the point of the timeout would show
  which one is missing.

- **B-104 A user cannot select a bundler adapter**
  `Units: cli · Size: M · Impact: Low`

  `defaultBundler` (`packages/cli/src/defaultBundler.mts`) wires in the Vite
  adapter, and the CLI picks it itself. How a user selects another adapter
  is not decided. One planned shape is an `@incajs/cli/rspack` entry.

- **B-105 `quickjs.rs` mirrors two gaps of rquickjs**
  `Units: jsenv · Size: S · Impact: Low`

  A panic stored by a Rust callback stays with the runtime until rquickjs
  resumes it, because the function that resumes it is private to rquickjs.
  `Value::as_proxy` returns `None` for a proxy whose target is callable, so
  `proxy_target` keeps one `unsafe` block. A fix in rquickjs would remove
  both.

- **B-106 Expose the app session to a framework test environment**
  `Units: host · Size: M · Impact: Low`

  A test environment that app authors use needs mocks and direct event
  firing. `Session` stays private until that environment is designed, and
  `Harness` covers what the host's own tests need.

- **B-107 A test protocol and Locator library for Playwright**
  `Units: host,cli · Size: L · Impact: Low`

  `snapshot`, `click`, `type` and `waitIdle` over a host protocol, plus a
  Node Locator library (`getByText`, `getByTestId`, auto-wait) under
  `@playwright/test`. Decide it together with B-005, which keeps the shipped
  binary's protocol surface small.

- **B-108 Screenshots in tests**
  `Units: host,ci · Size: L · Impact: Low`

  gpui renders headless on macOS only, and Linux has no offscreen renderer.
  Image comparison such as `toMatchSnapshot` needs one.

- **B-109 Role and accessible-name queries**
  `Units: gpui,host · Size: M · Impact: Low`

  gpui's accessibility tree stays inactive under the test platform. Role
  and name locators need it, or a role derived from the tree.

- **B-110 Real text shaping in the harness on macOS**
  `Units: host · Size: S · Impact: Low`

  Linux uses the platform text system headless. macOS needs a direct text
  system, so text widths there come from the fake one.

- **B-111 Text nodes have no bounds in a snapshot**
  `Units: gpui,host · Size: S · Impact: Low`

  Text leaves carry no selector, so a snapshot gives them no bounds.

- **B-112 Event identity is inferred from GPUI's per-node callbacks**
  `Units: bridge,gpui · Size: L · Impact: Low`

  GPUI calls the host once per node, so the host reconstructs each event:
  `eventId` is one number per event name per input, and the end of an input is
  the next `cx.defer`. A handler attached during a `mouseup` for the `click`
  of the same input can be skipped, because GPUI dispatches a child's `click`
  before its `mouseup`. A handler attached by a `mouseenter` handler runs for
  the `mousemove` of the same move, because hover is dispatched first inside
  that input and the `mousemove` name gets a later id. Two focus transitions
  in one update (JS calls `focus()` inside a handler) share the `focus` id, so
  a handler attached during the first is skipped for the second. Each node on
  the path gets a fresh event object, so a property a listener sets on it is
  invisible to the listeners of its ancestors. A host that builds one event,
  then walks the path and runs capture and bubble itself, gives every event a
  single identity and removes the inference, the edge case and the fresh
  object per node. It also takes over `stopPropagation` and `preventDefault` from GPUI.
  Decide it with the first adapter other than Vue.

- **B-113 A failed rebuild in `inca dev` prints the same error twice**
  `Units: cli · Size: S · Impact: Low`

  Renaming a source file during `inca dev` prints the same build failure
  twice, once for each rebuild the rename starts.

- **B-114 `inca dev` mixes the bundler's output with its own lines**
  `Units: cli · Size: S · Impact: Low`

  A failed rebuild prints the bundler's `build started...` line and its boxed
  error frame between the stamped `[inca]` lines, so one failure appears in
  two output styles.

- **B-115 The config loader's `jiti` hint suggests an unneeded install**
  `Units: cli · Size: S · Impact: Low`

  When a config file fails to load, `c12` appends `Hint install jiti for
  compatibility` to the file's own error, such as a syntax error. The hint
  suggests a package that does not apply to the failure.

- **B-116 `inca package` on Linux writes no icon and no `.desktop` file**
  `Units: cli · Size: M · Impact: Medium`

  `icon` takes a macOS `.icns` file, and only the macOS package uses it. A
  Linux package holds the host binary and the bundle, so a launcher or a task
  bar shows no icon. Linux needs an icon format such as PNG or SVG and a
  `.desktop` file that matches the window's `identifier`.

- **B-117 A known style key with a wrong-shaped value is dropped quietly**
  `Units: gpui · Size: S–M · Impact: Low`

  An unknown key and an invalid colour print a warning when the style is
  set. A known key whose value has the wrong type or range, such as
  `display: "nope"`, `gap: "1"`, a negative `padding` or a string `opacity`,
  is dropped and prints nothing.

- **B-118 A style warning prints again on every `setStyle` call**
  `Units: bridge · Size: S · Impact: Low`

  The warning is printed once per `setStyle` call. A reactive `:style` that
  sets a bad key on each patch prints a line each time, and a remount prints
  it again. Remembering the keys already reported per node would print each
  once.

- **B-119 `--print-config` prints the defaults for a malformed `inca.json`**
  `Units: host · Size: S · Impact: Low`

  The host reads `inca.json` through the same function as a normal start,
  which writes one `ignoring ...` line to stderr and continues with the
  defaults. A wrongly typed field, such as a string `width`, drops the whole
  file. `--print-config` then prints those defaults and exits 0. A
  diagnostic for "why is my window that size" could exit 1 and name the
  field or the syntax error.

- **B-123 The style vocabulary has no machine-readable definition**
  `Units: gpui,core,cli · Size: L · Impact: Low`

  `StyleProps` (`packages/core/src/types.mts`) copies the host's vocabulary
  by hand. A definition in the host that generates JSON and TypeScript types
  gives tools one source. The types let an editor check `:style` keys in the
  spelling the app wrote, and a Vite plugin can warn about static keys with
  file and line, with no host process at build time. The Tailwind class
  resolver of Phase 7 reads the same data, and B-001 builds on the types. It
  follows B-120 and B-121.

- **B-127 A boolean attribute set to `false` stays set**
  `Units: core · Size: S · Impact: Low`

  `patchProp` stores `false` as a boolean attribute for every name except
  `disabled`, `hidden` and `inert`, which it removes. Vue removes an
  attribute set to `false` for the names it lists as special boolean
  attributes, such as `readonly` and `novalidate`, sets `checked` as an
  element property, and keeps `false` as the text `"false"` for any other
  attribute. The input element decides which other names need this.

- **B-129 A focused node looks like an unfocused one**
  `Units: gpui,core · Size: L · Impact: Low`

  The `focus` and `blur` events are the only sign of focus. The fix adds a
  `:focus` state to the host's style model: a style variant that applies while
  the node holds focus, set from the same focus transitions that dispatch
  `focus` and `blur`.

- **B-131 Focus state is readable through events only**
  `Units: core,bridge · Size: M · Impact: Low`

  `document.activeElement` and `:focus` are unsupported. The fix adds a
  binding that returns the focused node id, and core wraps it as the focused
  element.

- **B-143 `disabled` has no handling for the input element**
  `Units: bridge · Size: S · Impact: Low`

  The input element (B-127) needs `disabled` handling when it lands. The fix
  adds the tag to the `disabled` check in `tab_index_of`.

- **B-141 `hidden="until-found"` renders and focuses as an unhidden node**
  `Units: gpui,bridge · Size: M · Impact: Low`

  The Hidden Until Found state keeps the box and skips painting its content
  (`content-visibility: hidden`), and its content cannot take focus. The fix
  adds a `content-visibility` style that skips painting and the focus
  handles of the subtree, and maps `until-found` to it.

- **B-142 `visibility` style is unsupported**
  `Units: gpui,bridge · Size: M · Impact: Low`

  `visibility: hidden` keeps the node painted and focusable. The fix adds a
  `visibility` prop that skips painting and the focus handle, and descendants
  inherit it unless they override it.

- **B-134 `focus()` ignores its options and leaves the scroll offset**
  `Units: bridge,core · Size: M · Impact: Low`

  `focus({ preventScroll })` ignores the argument, and focusing a node outside
  its scroll container keeps the offset. The fix adds a scroll-into-view
  operation to the host and calls it from `focus()` unless `preventScroll` is
  set.

- **B-136 `el.tabIndex` reads only the last assigned value**
  `Units: core,bridge · Size: M · Impact: Low`

  A `tabindex` set through a template prop is invisible to the getter, which
  returns `-1` (`0` for a button) until the property is assigned. The fix adds
  an attribute read binding to the host, `native.mts` and `rendererCore.mts`,
  and every mock of the native object gains it.

- **B-140 A container with `pointer-events: none` becomes `target`**
  `Units: gpui,core · Size: M · Impact: Low`

  Every container under the pointer can become `target`, whatever its style.
  Once a `pointer_events` style key exists, a container that sets `none`
  skips its target marker and its hitbox, so the next container below reports
  itself.

- **B-149 Tab after a click starts at the first stop**
  `Units: gpui,bridge · Size: M · Impact: Low`

  A click on an empty area leaves nothing focused, so the next Tab goes to the
  first stop. Browsers continue from the clicked position. The fix keeps a
  starting-point node set by the mouse target and clears it when focus moves
  by another route. `tab_move` starts from it when nothing is focused.

- **B-151 `beforeinput` is unsupported**
  `Units: gpui,bridge,docs · Size: L · Impact: Low`

  A `beforeinput` listener has no effect. Fix: after the text-editing unit
  lands, dispatch it before each edit as a cancelable event with `data`,
  `inputType` and `isComposing`, and apply the edit when no listener calls
  `preventDefault()`.

- **B-152 `contextmenu` has no keyboard trigger**
  `Units: gpui,bridge · Size: S-M · Impact: Medium`

  `contextmenu` fires only from a secondary-button press. The Menu key and
  Shift+F10 have no effect. Fix: the key hook dispatches it at the focused
  node with the keyboard pointer source.

- **B-153 `KeyboardEvent` has no `keyCode`, `which` or `charCode`**
  `Units: bridge · Size: S-M · Impact: Medium`

  `keydown` and `keyup` carry `key` and no legacy code fields. Fix: a table
  from `key` to `keyCode` in `set_key_fields`, with `which` equal to
  `keyCode`.

- **B-154 `input` and `change` are unsupported**
  `Units: gpui,bridge,docs · Size: L · Impact: High`

  `input` and `change` listeners have no effect. Fix: dispatch them from the
  text-editing unit together with `beforeinput` (B-151).

- **B-155 Drag-and-drop events are unsupported**
  `Units: gpui,bridge,docs · Size: L · Impact: Medium`

  `dragstart`, `drag`, `dragover`, `drop` and `dragend` listeners have no
  effect. Fix: start a drag from a `draggable` node on pointer movement and
  dispatch the sequence to the source and the drop target.

- **B-156 Touch events are unsupported**
  `Units: gpui,bridge,docs · Size: L · Impact: Low`

  `touchstart`, `touchmove`, `touchend` and `touchcancel` listeners have no
  effect, and a touch tap produces a `click` only. Fix: dispatch the touch
  sequence from `gpui` touch input with `touches` and `changedTouches`.

- **B-157 Nodes lack `dispatchEvent()` and `click()`**
  `Units: core,bridge · Size: M-L · Impact: Low-Medium`

  Listeners register through the native `addEventListener` only, and app code
  has no call that dispatches an event on a node. Fix: expose `dispatchEvent()`
  and `click()` on the element wrapper and route them to the host dispatcher.

- **B-158 Keys with nothing focused reach only the host root, which templates cannot listen on**
  `Units: core,bridge,docs · Size: M · Impact: Medium`

  With nothing focused, `keydown` and `keyup` target the host root node. A
  listener on the app's top element is a child of that node and never runs, so
  an app cannot react to keys while nothing is focused. Fix: expose a
  listener target for the root, such as `window` or `document`, through
  `addEventListener` on the element wrapper of the root.

- **B-159 `inca dev --experimental-hmr` prints Vue's feature flag warning**
  `Units: cli · Size: S · Impact: Low`

  The development server prints a warning that `__VUE_OPTIONS_API__`,
  `__VUE_PROD_DEVTOOLS__` and `__VUE_PROD_HYDRATION_MISMATCH_DETAILS__` are not
  defined. Fix: define the three flags in the Vite configurations of the
  development server and the build.
