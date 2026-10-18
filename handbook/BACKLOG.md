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

- **B-121 Style keys and values use the host's vocabulary**
  `Units: gpui,core,docs · Size: L · Impact: High`

  An app writes `:style` with the host's names and values, such as
  `textColor`, `background`, `cornerRadius`, `textSize` and
  `justifyContent: "start"`, with sizes as numbers of px. A Vue developer
  knows `color`, `backgroundColor`, `borderRadius`, `fontSize`,
  `justifyContent: "flex-start"` and `"10px"`. Goal: an app writes standard
  CSS in `:style` and meets no host vocabulary. The host maps each CSS
  property and value form to its own style, the reference lists the CSS
  names, and the host's own names stay accepted as aliases. A warning names
  the key as written. CSS accepts key spellings that the host warns about:
  uppercase or mixed-case kebab keys such as `Flex-Direction`, vendor
  prefixes and `--custom` properties. B-085 (percentages, shorthands) and
  B-117 (wrong-shaped values) belong to the same change. B-123 generates the
  types from the same mapping.

- **B-123 The style vocabulary has no machine-readable definition**
  `Units: gpui,core,cli · Size: L · Impact: Medium`

  `StyleProps` (`packages/core/src/types.mts`) copies the host's vocabulary
  by hand and lists snake_case keys. Vue's `CSSProperties` type lists only CSS
  properties, so an editor that checks `:style` flags the host's own keys. A
  definition in the host that generates JSON and TypeScript types gives tools
  one source. The types let an editor check `:style` keys, and a Vite plugin
  can warn about static keys with file and line during the build. The
  Tailwind class resolver of Phase 7 reads the same data, and B-001 builds on
  the types. It follows B-121 and generates from its CSS mapping.

## P2

- **B-007 `.vue` `<style>` blocks don't reach the screen**
  `Units: cli,core,gpui · Size: L · Impact: Medium`

  Scoped/global
  `<style>` blocks in `.vue` SFCs have no effect through the custom
  renderer. A build now fails on one rather than shipping it
  (`ERR_INCA_UNSUPPORTED_STYLE`, raised by `vite/unsupported.mts`);
  rendering them is Phase 7's work.

- **B-012 The window and the app control each other only at start**
  `Units: bridge,host,core · Size: L · Impact: Medium`

  The host takes the window's size and title from the config and the
  mounted root. Each size axis fits once, when its content size first
  becomes known (`open()` and `maybe_auto_resize_to_content` in
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

- **B-028 A template `ref` lacks most element members**
  `Units: core,bridge · Size: L · Impact: Medium`

  The element handle lacks:
  - the attribute members (`getAttribute()`, `setAttribute()`,
    `hasAttribute()`, `removeAttribute()`, `attributes`);
  - `classList`, `className`, `dataset`, `style`, `tagName`;
  - the tree members (`parentElement`, `firstChild`, `nextSibling`,
    `closest()`).

  The members it has differ from a browser in the box reads (B-160), the
  `offsetParent` chain (B-161), the scroll members (B-172, B-173, B-174) and
  `scroll-margin` (B-176). Its own `children` field lists text nodes too, and
  the DOM `children` lists elements only.

  Fix: build the attribute members on the `getAttribute` binding, and the tree
  members on the JS shadow tree.

- **B-036 Reverting a script edit under `--experimental-hmr` sometimes applies nothing**
  `Units: cli · Size: M · Impact: Medium · Status: needs repro`

  Editing `onClick` in `examples/click_counter` from `clicks.value += 1` to
  `+= 2` and then `+= 3` applies each value on the next click. Editing back
  to `+= 1` sometimes leaves the app on `+= 3`: the host receives nothing,
  the count keeps its value and the increment stays 3. A later style edit
  applies the style only. Linux and macOS both showed runs that applied the
  revert and runs that left it.

  A Vite dev server with `@vitejs/plugin-vue` delivers every revert: 18 of
  18 edits over six sessions, including reverts to the startup content. The
  loss sits in the CLI pipeline (`fileChangedPlugin`, the module runner, the
  50 ms merge of saves) or between the editor save and the watcher. A repro
  records the terminal output and the time between saves.

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

- **B-097 `console` prints some classes and built-ins unlike Node**
  `Units: jsenv · Size: M · Impact: Medium`

  Properties keyed by a `Symbol` and extra properties on an array or an
  `Error` are dropped. A `Promise`, a typed array, a `WeakMap` and a boxed
  primitive print as a plain object: `{}` for most, `{ '0': 0, '1': 0,
  '2': 0 }` for `new Uint8Array(3)`, where Node prints `Promise { 1 }` and
  `Uint8Array(3) [ 0, 0, 0 ]`. A nested string, and an object key that is
  not an identifier, is always single-quoted, where Node picks the quote
  that avoids escaping.

- **B-122 Colors take fewer notations than CSS**
  `Units: gpui,docs · Size: M–L · Impact: Medium`

  A color is a `0xRRGGBB` number or a `#rgb` or `#rrggbb` string. CSS adds,
  from small to large: `#rgba` and `#rrggbbaa`, `transparent` and
  `currentcolor`, `rgb()`, `rgba()`, `hsl()` and `hsla()` in the comma and
  the space syntax with an alpha, the 148 color names, `hwb()`, `lab()`,
  `lch()`, `oklab()`, `oklch()`, `color()`, `color-mix()`, relative colors,
  `light-dark()` and system colors. `var(--x)` waits for CSS variables.
  gpui colors carry an alpha, so the parser sets the limit. A parsing crate
  such as `csscolorparser` covers part of the list, and its range needs a
  check before use. Only `background`, `border_color` and `text_color` take a
  color. `outline-color`, per-side border colors, `box-shadow` and
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

- **B-187 The bridge globals can be overwritten by the app**
  `Units: bridge,jsenv,host,core · Size: M · Impact: Medium`

  `__inca_native__`, `__inca_dev__`, `__inca_callbacks__`,
  `__inca_released__`, `__inca_targets__`, `__inca_track_rejection__` and
  `__inca_take_rejections__` are plain globals. A script can assign to or
  delete them, which breaks the host channel or the report of unhandled
  rejections.

  Fix: define them with `Object::prop(key, value)` of rquickjs, which creates
  a read-only property that stays in place. The JS side creates
  `__inca_callbacks__`, `__inca_released__` and `__inca_targets__` (`??=` in
  `callbacks.mts` and `eventTarget.mts`, so the first copy of core wins), so
  the host creates and protects them first.

- **B-188 The pending module holds a promise handle**
  `Units: jsenv · Size: M · Impact: Medium`

  `PendingModule` in `engine.rs` keeps a `Persistent<Promise>` while the
  top-level `await` of a module is pending. Rust keeps ids and plain data,
  and a handle kept past the call that produced it outlives its context
  (listener ids are the model, see FFI.md "JS values stay in JS"). A fix
  keeps the promise in JS and gives Rust an id.

- **B-189 The event store holds JS values in Rust**
  `Units: bridge · Size: M · Impact: Medium`

  The event store of `event.rs` keeps the record `WeakMap`, the three
  prototypes and two functions as userdata of the context. They are JS values
  held by Rust. A fix keeps them in JS and lets Rust refer to them by id. The
  mouse and keyboard prototypes move with the rest, and the `kind` of each
  event record, a slot index, becomes the id of its prototype. B-185
  covers the shape of the same store.

- **B-195 `removeEventListener` accepts a malformed `callbackId`**
  `Units: bridge,docs · Size: S · Impact: Medium`

  `removeEventListener` takes a plain `u32`, and rquickjs converts a number
  through `f64` with a range check only. `1.5` becomes 1 and `NaN` becomes 0,
  so the call removes another registration of that node and event instead of
  throwing the `TypeError` that `FFI.md` promises. Found by reading; not
  reproduced.

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
  The same typing covers event names: a misspelled `@click`, or a `v-on`
  object key such as `onMousedown`, fails the check, and custom event names
  keep a `string` entry.

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
  a color, so it can't fill what a fixed-size root in a larger window
  leaves uncovered. A background color needs a full-size wrapper node in
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

- **B-033 `[inca] reload`/`[inca] update`'s timing isn't the useful number**
  `Units: cli,host · Size: M · Impact: Low`

  both only measure how long the bundler took to prepare an update, not
  how long until the app actually applies it and the screen reflects
  it. Getting the real number needs the host to confirm back to Node
  once an update is actually applied, which no dev-protocol message
  does today.

- **B-035 A `full-reload` HMR payload logs nothing on success**
  `Units: cli · Size: S · Impact: Low`

  `dev.mts`'s `reload: () => void reloadHost()` (passed to `bundler.hmr`)
  logs no line on success, so a full reload under `--experimental-hmr` is
  invisible to the user. `onBuild` logs a `reload ... (Nms)` line for the
  same step.

- **B-037 Dev relay hardening (`__inca_dev__`)**
  `Units: bridge,host · Size: S–M · Impact: Low`

  Three related gaps in `crates/inca-host/src/dev/globals.rs`. `install_dev`'s
  `send` accepts any method name, including the host's own reserved ones
  (`ready`, `appError`), so a running app can spoof a host notification.
  `call_dev_receive` discards whatever `.ok()?` swallows when reading
  `__inca_dev__`/`receive` off globals, rather than capturing it as an `EngineError`. `receive` is
  invoked via a bare `Function::call`, leaving `this` unbound instead of
  bound to `__inca_dev__` — `handbook/PROTOCOL.md`'s own
  `__inca_dev__.receive?.(...)` implies a method call. Also,
  `PROTOCOL_VERSION` (`crates/inca-host/src/dev/wire.rs`) is still `0`
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

- **B-074 Dev and production differ in `TextDecoder`, `URL` and the `define` globals**
  `Units: cli · Size: S · Impact: Low`

  With `--experimental-hmr` the runtime installs a Latin-1 `TextDecoder`, a
  minimal `URL` and, on the app's `globalThis`, each `define` value whose key
  is a plain identifier and whose code is a JSON literal. A production build
  inlines the `define` values and has none of these globals, so dev and
  production differ.

- **B-075 A failed host start in inca dev is handled differently per mode, and the checks overlap**
  `Units: cli · Size: S–M · Impact: Low`

  `dev()` stops right away when no host binary resolves or the path is not a
  file. `startHost` handles every other start failure. Without HMR it prints
  "failed to start the host", keeps running and retries on the next rebuild.
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

- **B-079 Scrollbars are not supported**
  `Units: gpui · Size: M · Impact: Low`

  The DOM shows a scrollbar for `overflow: scroll`. Here `gpui`'s scrollbar
  width stays 0, so the wheel is the only way to scroll.

- **B-082 Wheel scrolling has `gpui`'s axis rules and no gesture latching**
  `Units: gpui · Size: M · Impact: Low`

  A vertical wheel scrolls a container that scrolls only on x, and only the
  dominant axis of a diagonal delta moves. The rules are `axis_step` in
  `crates/inca-gpui/src/element/scroll.rs`. The DOM latches a continuing
  gesture to the container it started on. Here a gesture moves to the parent
  as soon as the inner container reaches its limit.

- **B-084 Wheel scrolling differs from the DOM at the edges**
  `Units: gpui,bridge · Size: M · Impact: Low`

  There is no `overscroll-behavior` and no rubber-banding. Line deltas are
  scaled by `gpui`'s line height, which differs from the browser's.

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
  `%o` follows Node with `showProxy` off, but leaves out the hidden
  properties of a function and of an `Error`, such as `[stack]` and
  `[message]`. Fix: list the non-enumerable own properties of a function and
  of an `Error`. `%c` applies the `color` declaration only, with the basic
  color names, and the Console Standard leaves `%c` undefined. Fix: map the
  CSS properties `background-color`, `font-weight`, `font-style` and
  `text-decoration` to terminal styles, and accept hex, `rgb()` and `hsl()`
  values of `color`. A conversion that throws prints the value as `dir` does,
  where the standard throws. A `Symbol` or a label whose `toString` throws
  becomes the default label where the standard throws. `time`, `timeLog` and
  `timeEnd` always print milliseconds. `dir` ignores its `options`. `dirxml`
  formats like `log`. `clear` closes the open groups and leaves the terminal
  as it is. `groupCollapsed` prints its lines like `group`, since a terminal
  cannot fold them. `table` ignores a `columns` argument that is not an array,
  and drops properties keyed by a `Symbol` and a `Symbol` column name. `trace`
  reads its stack through the global `Error`, which a script can replace.

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
  - A `RegExp` has one color, where the standard highlights its parts.
  - `%s`, `%d`, `%i` and `%f` convert an array that holds a `Proxy` through
    the array's own string form, which runs the `Proxy`'s traps.

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

  An unknown key and an invalid color print a warning when the style is
  set. A known key whose value has the wrong type or range, such as
  `display: "nope"`, `gap: "1"`, a negative `padding` or a string `opacity`,
  is dropped and prints nothing.

- **B-118 A style warning prints again on every `setStyle` call**
  `Units: bridge · Size: S · Impact: Low`

  The warning is printed once per `setStyle` call. A reactive `:style` that
  sets a bad key on each patch prints a line each time, and a remount prints
  it again. A removal in a patch resends every remaining entry, so each bad
  key warns again. Remembering the keys already reported per node would
  print each once.

- **B-119 `--print-config` prints the defaults for a malformed `inca.json`**
  `Units: host · Size: S · Impact: Low`

  The host reads `inca.json` through the same function as a normal start,
  which writes one `ignoring ...` line to stderr and continues with the
  defaults. A wrongly typed field, such as a string `width`, drops the whole
  file. `--print-config` then prints those defaults and exits 0. A
  diagnostic for "why is my window that size" could exit 1 and name the
  field or the syntax error.

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

  A focused node has no visual sign. The fix adds a
  `:focus` state to the host's style model: a style variant that applies while
  the node holds focus, set from the same focus transitions that dispatch
  `focus` and `blur`.

- **B-131 `document.activeElement` and `:focus` are unsupported**
  `Units: core · Size: S · Impact: Low`

  `activeElement()` from `incajs` returns the focused element. Fix:
  expose it as `document.activeElement` once the `document` of the event plan
  exists, and add `:focus` with the interactive visual state.

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
  effect, and a touch tap produces the mouse events and a `click`. A long
  press fires nothing. Fix: dispatch the touch sequence from `gpui` touch
  input with `touches` and `changedTouches`.

- **B-160 Element reads see the last drawn frame**
  `Units: gpui,bridge,core · Size: L · Impact: Medium`

  `getBoundingClientRect()`, the `offset*` and `client*` members and the
  `offsetX` and `offsetY` of mouse events use the box of the last drawn
  frame. A node created or changed in the same task reads as before the
  change, or as zeros. A read in `onMounted` gets zeros. A keyboard click has
  the client position `(0, 0)`, so its offsets count from there. Fix: a layout
  pass the bindings can call, or a notification after each frame, which fits
  the rule that JS runs only on change.

- **B-161 Offsets of a template `ref` differ from the browser**
  `Units: core · Size: S · Impact: Low`

  `offsetParent` is always the direct parent. A browser uses the nearest
  positioned ancestor, and the offsets measure from it. Fix: pick the nearest
  ancestor with a `position` once that style exists.

- **B-162 Style warnings print as plain text outside the console**
  `Units: gpui,bridge,jsenv · Size: L · Impact: Low`

  `console` prints decorated lines, with colors, class names and the detail of
  `%o`. A style warning such as ``node 1 (div): ignoring unknown style key
  `justifyContent` `` is a plain line written by `eprintln!`, so it cannot use
  that output. The formatting code lives in `inca-jsenv`. Fix: move the
  output formatting into a crate that `inca-jsenv`, `inca-bridge` and
  `inca-gpui` depend on, so JS and Rust print through one path. A smaller
  first step is to route the bridge's style warnings through the console
  output, because `inca-bridge` already depends on `inca-jsenv`.

- **B-163 `--experimental-hmr` starts the host before the app's modules are transformed**
  `Units: cli · Size: S–M · Impact: Low`

  The development server creates the Vite server and starts the host at once.
  The host then fetches each module of the app through the `vite` relay and
  evaluates it, and the development window opens once the app has mounted. A
  measured start spends about 0.3 s between the host starting and the window
  opening. Fix: before `startHost`, call the Vite server's `warmupRequest`
  for the entry so the transform time passes before the host exists. A
  transform failure then shows before any host starts, the same as a failed
  first build without HMR. Evaluation and mount errors still need the host's
  wait.

- **B-164 Gaps in the host's wait for the first load**
  `Units: host · Size: S · Impact: Low`

  Two gaps remain in `crates/inca-host/src/dev.rs`. While a reload is pending,
  relayed notifications go to the pending session until it settles. When the
  first entry rejects while no window exists, the host exits right after it
  reports the rejection, so the job failures of the final poll are dropped.

- **B-165 A stale `DISPLAY` makes the host panic on Linux**
  `Units: host · Size: S · Impact: Low`

  The host connects to X11 when `DISPLAY` is set and to Wayland when
  `WAYLAND_DISPLAY` is set. A variable that points to no running server ends
  the host with a panic and exit code 101, and the message names only
  `Failed to initialize X11 client`. A shell left over from an SSH session
  or a closed X server shows this. Fix: check the connection before the
  window opens and exit 1 with a message that names the variable and tells
  the user to unset it.

- **B-167 Only `Event` and `CustomEvent` can be constructed**
  `Units: bridge,core,docs · Size: M · Impact: Low`

  A script cannot build a `MouseEvent`, `KeyboardEvent`, `FocusEvent`,
  `WheelEvent`, `UIEvent` or `PointerEvent`. `Event` has no `initEvent()` and
  `CustomEvent` no `initCustomEvent()`. Mouse, wheel, key and focus events
  report `Event` as their `constructor` and `[object Event]` as their string
  tag, where browsers report their subclass. One mouse prototype serves mouse
  and wheel events, so it carries no tag of its own. Fix: add the subclasses
  with their init dictionaries, `initEvent()`, `initCustomEvent()` and string
  tags, and have the host's events inherit from them.

- **B-168 `window` has the listener methods only and `document` is undefined**
  `Units: core,bridge,docs · Size: M · Impact: Low`

  `window` is a named export of `incajs` with `addEventListener()`,
  `removeEventListener()` and `dispatchEvent()` as its only members, so it
  lacks the handler properties such as `onkeydown`. The globals
  `window` and `document` are undefined, and the `signal` option of
  `addEventListener()` is unsupported. `composedPath()` ends at `window` and
  has no `document`. Activating or leaving the window fires `focus` and `blur`
  at the focused node only, where browsers also fire them at `window`. Fix:
  install a global `document` and decide how a global `window` coexists with
  the `typeof window` checks of libraries. The `signal` option needs an
  `AbortController` in the runtime first.

- **B-169 The Vue warning trace prints in the text color**
  `Units: core · Size: S · Impact: Low`

  The frames of an `Error` value print dimmed. Vue writes the component trace
  of a warning (`at <App>`) as plain string arguments of `console.warn`, so
  the host cannot tell it from other text, and `inca-jsenv` must not know
  Vue. Fix: the Vue adapter sets a default `app.config.warnHandler` that
  receives the message and the trace as separate values and prints the trace
  through `%c` with a gray `color`. The handler an app sets itself takes
  precedence.

- **B-170 The handles of a removed element are inert**
  `Units: core · Size: S · Impact: Low`

  Vue destroys a node on unmount, and the host frees a detached node that
  nothing holds. Listeners registered before are released,
  `addEventListener()` afterwards ignores the listener, an assignment to a
  handler property does nothing, `dispatchEvent()` returns `true` and skips
  the argument check, `tabIndex` reads the default and `id` reads an empty
  string. A browser keeps the listeners and the attribute of a detached
  element. Fix: keep a destroyed node as a detached element in the host tree.

- **B-171 A frozen event cannot be dispatched again**
  `Units: bridge,docs · Size: M · Impact: Low`

  Dispatching sets `target`, `isTrusted` and `eventId` on the event object,
  which throws a `TypeError` on a frozen object. Browsers dispatch a frozen
  event. Fix: keep `target`, `currentTarget`, `eventPhase`, `isTrusted` and
  `eventId` in the event record behind accessors on the prototype. Browsers
  define `isTrusted` as an own getter of each event, enumerable and not
  configurable, so its accessor goes on the event object.

- **B-172 Scroll members of a template `ref` read the last drawn layout**
  `Units: gpui,bridge,core · Size: M · Impact: Low`

  The scroll maxima come from the last drawn frame. Two differences from a
  browser remain:
  - An offset set beyond the end, or before the first draw, is clamped at the
    next frame, so a read right after the set returns a smaller offset. A
    browser clamps at once.
  - For a `visible` or `clip` element, `scrollWidth` and `scrollHeight` equal
    the client size. A browser reports the size of the content, overflow
    included.

  Fix: a layout pass the bindings can call, as in B-160. Both need the layout
  of the current tree.

- **B-173 The scroll range of a container ends before its bottom padding**
  `Units: gpui · Size: M · Impact: Low`

  At the maximum offset the last child stops short of the client bottom by the
  `padding-bottom` of the container, and `scrollHeight` leaves that padding
  out. A browser includes `padding-bottom` in the scroll range.

  Fix: `gpui` derives the content height from the children. The range needs
  the trailing padding added, for example through a spacer child of that
  height.

- **B-174 `behavior: "smooth"` scrolls jump**
  `Units: gpui,bridge,core · Size: M · Impact: Low`

  `scrollTo`, `scrollBy` and `scrollIntoView` apply every scroll at once. A
  browser animates a `smooth` scroll.

  Fix: an animation the host drives per frame, because the JS runtime runs
  only when reactive state changes.

- **B-175 The `scrollend` event is missing**
  `Units: gpui,bridge · Size: M · Impact: Low`

  Chromium and Gecko fire `scrollend` once a scroll has settled. The host fires
  `scroll` alone. A script write can fire `scrollend` in the same pass right
  after `scroll`. A wheel scroll needs a time threshold, because no frame
  follows the last wheel step and a quiet frame in the middle of a gesture
  would end it early.

  Fix: a `gpui` executor timer that fires `scrollend` after the threshold, with
  a rule for trackpad momentum. The threshold and the rule are design
  decisions.

- **B-176 `scroll-margin` and `scroll-padding` are missing**
  `Units: core,bridge · Size: M · Impact: Low`

  A browser keeps a node that `scrollIntoView()` or focus brings into view a
  `scroll-margin` away from the container edge, and a container's
  `scroll-padding` sets the edge that scrolling stops at. The host scrolls the
  node flush with the container edge in both cases.

  Fix: add `scroll-margin` on nodes and `scroll-padding` on containers as
  style keys, and subtract them from the visible box in the reveal geometry
  before aligning.

- **B-177 A root taller than the window is cut off**
  `Units: gpui,host · Size: M · Impact: Medium`

  The host draws the root node with the default `overflow: visible`, so content
  below the window edge is clipped, and the wheel moves it only inside a
  container with `overflow` set to `scroll` or `auto`. A browser scrolls the
  document, so the same content scrolls under the wheel and under focus.

  Fix: give the host root a scroll handle (`overflow: auto`) with an opt-out.
  That also lets focus and `scrollIntoView` reach content below the window. The
  default and the opt-out are design decisions for the owner.

- **B-178 A queued scroll into view waits for any later frame**
  `Units: bridge · Size: S–M · Impact: Low`

  `scrollIntoView()` on a node with no box yet waits for the next frame. When
  no change requests a frame, such as a node that stays at `display: none`,
  the call stays queued and scrolls whenever a later frame draws. A browser
  scrolls at once, because layout is synchronous.

  Fix: request an animation frame when the call is queued, and give the host
  binding the window handle it needs for that.

- **B-179 A recompile of a module name replaces its source map**
  `Units: jsenv · Size: M · Impact: Low`

  The source map registry keys each entry by filename. Under
  `--experimental-hmr`, a recompile of the same module replaces its source and
  map. A frame of a function from the previous compile that is still alive,
  such as an old listener or timer, maps through the new map and names a wrong
  line and column. A browser maps a frame through the map of its own script.

  Fix: key each entry by a per-compile generation in the filename the engine
  sees, and print the original filename.

- **B-182 The event store helper lives in the bridge**
  `Units: bridge,jsenv · Size: S · Impact: Low`

  `event.rs` builds a private `WeakMap` store by evaluating a script. The
  store needs only QuickJS, so it fits jsenv beside the other realm helpers.
  The planned native event record replaces the store, so the move applies
  only if that record is dropped.

- **B-183 Event fields can be reassigned**
  `Units: bridge · Size: M · Impact: Low`

  Fields such as `ctrlKey`, `key` and `clientX` are writable data properties
  of the event object. `getModifierState()` reads the modifier fields, so a
  reassigned field changes its result. `Object::prop(key, value)` of rquickjs
  defines a read-only property.

- **B-185 The event store keeps its pieces by index**
  `Units: bridge · Size: S · Impact: Low`

  `event.rs` keeps the `WeakMap` record store, the three prototypes and two
  functions in a `Vec<Value>` addressed by index constants. The `kind` of an
  event record holds one of these indexes. A struct with named fields needs
  `JsLifetime`. The `macro` feature of rquickjs provides
  `#[derive(JsLifetime)]`, which generates the `unsafe impl` and checks at
  compile time that every field type is valid, so the bridge keeps no
  hand-written `unsafe`. With `"macro"` in the workspace rquickjs features,
  `cargo check -p inca-bridge` compiles and `Cargo.lock` gains
  `rquickjs-macro` and its dependencies. `clippy -D warnings` is unchecked.
  The workspace sets `unsafe_code = "deny"` and keeps all `unsafe` in
  `inca-jsenv/src/quickjs.rs`, so the policy for macro-generated `unsafe` is
  decided first.

- **B-186 `Plain` implements `JsLifetime` by hand**
  `Units: jsenv · Size: S · Impact: Low`

  `quickjs.rs` holds the one hand-written `unsafe impl JsLifetime`, for
  `Plain<T>`. Check whether `#[derive(JsLifetime)]` (see B-185) can replace
  it and shrink the hand-written `unsafe` surface.

- **B-191 The event walk allocates on every event**
  `Units: bridge · Size: M · Impact: Low`

  `path_from` in `event/walk.rs` returns a `Vec` for each event, and a walk
  that reaches a listener builds one array of target objects in JS. Borrowing
  the tree path instead would hold a `RefCell` borrow across the engine call,
  which can panic. A fix has to end the borrow before the call.

- **B-192 Natives treat a missing node in two ways**
  `Units: bridge,docs · Size: S · Impact: Low`

  `focusNode`, `blurNode`, `removeEventListener` and `click` ignore a node
  that is gone. The other natives throw a `TypeError`. State the rule in
  `FFI.md`. `sync` in `patchProp.mts` checks `isDestroyed` before
  `addEventListener` for this reason; a rule that ignores a gone node makes
  that check unnecessary.

- **B-193 Optional native parameters use two types**
  `Units: bridge · Size: S · Impact: Low`

  `Opt<T>` passes `undefined` and `null` to `T::from_js`, and `Option<T>`
  maps both to `None`. The natives use both. Pick one type for each
  parameter by the handling `null` should get.

- **B-194 Protocol lines produced during quit teardown are dropped**
  `Units: host · Size: S · Impact: Low`

  `close_writer_on_quit` closes the dev stdout writer in a gpui quit observer,
  before gpui clears its windows and flushes effects. A line produced in that
  teardown reaches a closed channel and is logged and dropped. The writer's
  join has no timeout either, so a parent that stopped reading a full stdout
  pipe makes the quit hang. `SHUTDOWN_TIMEOUT` bounds only the futures. Found
  by reading; not reproduced.

- **B-196 `#[gpui_cases]` drops parts of the function signature**
  `Units: gpui,bridge · Size: S · Impact: Low`

  `expand` rebuilds the case function from its name, parameters, return type
  and body only. `async`, `unsafe`, generics, lifetimes and a `where` clause
  are lost, so such a test fails to compile. The README does not say so.

- **B-197 The panic hook can hang behind a blocked stdout writer**
  `Units: host · Size: S · Impact: Low`

  The `StdoutWriter` thread holds the stdout lock while it writes a line. A
  parent that stops reading a full stdout pipe blocks that write, and the
  panic hook of any other thread then waits for the same lock. The panicking
  thread neither exits nor aborts. The panic message already reached stderr.
  Found by reading; not reproduced.

- **B-198 Chromium fires `blur` and `focusout` when a focused node is removed or moved**
  `Units: bridge,docs · Size: S · Impact: Low`

  Chromium 153 fires `blur` and `focusout` at the removed or moved node, at the
  call. Firefox 155 and WebKit 26.6 fire neither. The host drops the focus at
  the call and fires neither, like Firefox and WebKit. Measured in
  `tmp/browser-check/focus-detach-results.md`. A fix fires the pair at the call
  and needs a decision on the engine to follow.

- **B-199 `target` is `window` where browsers report `body`**
  `Units: gpui,bridge,core,docs · Size: M–L · Impact: Low`

  A key event while nothing is focused has `window` as `target`, and its
  `composedPath()` is `[window]`. Browsers report `body` as `target`, with
  `html`, `document` and `window` above it. A `window` listener there reads
  `eventPhase` 1 in capture and 3 in bubble, and the host gives 2.

  A mouse press or click outside every element has `window` as `target`.
  Chromium and WebKit report `body`, and Firefox reports `html`. Moving the
  pointer from an element to an empty area fires `mouseout` and `mouseleave`
  with `window` as `relatedTarget`, and `mouseover` at `window`. Chromium and
  WebKit give `body` in both places, and Firefox gives `html`.

  The host root is the only node above the app. Fix: a host-owned body node
  under the root that the app mounts into and that takes these events. The
  same node fixes B-200.

- **B-200 The host root has two objects, `window` and the mount container**
  `Units: core,bridge · Size: M · Impact: Low`

  `app.mount()` mounts on the host root, and both the mount container handle
  and `window` stand for that node. Events carry `window` for it, so a
  listener added through the container handle reads `window` as
  `currentTarget`, and the `parent` of the app's top element is a handle
  that `composedPath()` never lists. Fix: the body node of B-199, so the
  container is a node of its own.

- **B-202 Chromium drops a wheel step the host passes on**
  `Units: gpui,bridge · Size: S · Impact: Low`

  Chromium sometimes drops the wheel step when a listener moves an inner
  container to its end: with a listener on the outer element, a passive
  document listener, or a document listener in some layouts. The host passes
  the step to the outer container, as Firefox and WebKit do. Fix: none until
  Chromium's rule is specified; it varies with the listener position and
  layout.

- **B-203 Chromium and the host give the step to different containers**
  `Units: gpui,bridge · Size: S · Impact: Low`

  When a listener moves an inner container off its end, Chromium gives the step
  to the outer container. The host gives it to the inner container, as Firefox
  and WebKit do. With a listener on the document all three engines give the
  step to the inner container. Fix: none until Chromium's rule is specified;
  it varies with the listener position and layout.

- **B-207 An event handler attribute runs nothing**
  `Units: core · Size: L · Impact: Low`

  An attribute that holds code, such as `onclick="count++"`, stays a plain
  attribute and runs nothing, where a browser compiles it into the element's
  handler. Fix: needs a decision on running code from strings.
