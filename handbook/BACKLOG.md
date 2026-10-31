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

- **B-060 A packaged app keeps no record of an error it raised**
  `Units: host · Size: S · Impact: Medium`

  An error a listener throws, an unhandled rejection, a failed first load and
  a stop by an engine limit go to stderr, which a packaged app does not show.
  Under `inca dev` they are reported as `appError`. Fix: write them to a
  second file next to `panic.log`, from the reporter of the production host.

- **B-121 `:style` leaves out some CSS value and key forms**
  `Units: gpui · Size: S · Impact: Low`

  `:style` takes `px` as the only unit, so `"1em"`, `"10pt"` and `"50%"` for
  a size warn. `opacity` takes a decimal string but not `"50%"`. `"1e999px"`
  warns, and CSS accepts it. A key that starts with `-` is unknown, so vendor
  prefixes (`-webkit-flex`) and custom properties (`--gap`) warn, and CSS
  accepts both. Percentages for sizes and shorthands are B-085.

- **B-123 The style vocabulary is not emitted as JSON and TypeScript types**
  `Units: gpui,core,cli · Size: L · Impact: Medium`

  `StyleProps` (`packages/core/src/types.mts`) copies the host's vocabulary
  by hand. Vue's `CSSProperties` type lists only CSS properties, so an
  editor that checks `:style` flags the host's own keys.
  `inca_gpui::STYLE_KEYS` lists the keys, their CSS names, value kinds and
  keyword names, but no description, and the padding and margin keys carry
  no precedence. A generator that writes JSON and TypeScript types from it,
  with descriptions added to the declaration, gives tools one source. It
  also needs the range of each kind as data, the synonyms inside a keyword
  list (`auto` and `scroll`), and a choice of the spellings the types
  expose: the host takes camelCase, kebab-case and snake_case keys at run
  time. The types let an editor check `:style` keys, and a Vite plugin
  can warn about static keys with file and line during the build. The
  Tailwind class resolver of Phase 7 reads the same data, and B-001 builds on
  the types. It follows B-121, which leaves out some CSS value and key forms.

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
  `offsetParent` chain (B-161), the scroll members (B-172, B-174, B-216 and
  B-217) and `scroll-margin` (B-176). Its own `children` field lists text nodes
  too, and the DOM `children` lists elements only.

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

- **B-053 `focus`/`blur` listeners that call `focusNode` may not take effect**
  `Units: bridge,host · Size: S–M · Impact: Medium`

  `focus` and `blur` listeners run inside `render()`, where GPUI ignores
  `window.refresh()` (`third_party/zed/crates/gpui/src/window.rs:2178`), so
  the request may wait for an unrelated redraw. Unconfirmed.

- **B-059 The host sets no time limit, and nothing follows a stop by an engine limit**
  `Units: jsenv,host · Size: M · Impact: Medium`

  The engine can stop a call after the time of `Limits::time`, and the host
  sets none. That stop skips `catch` and `finally`: a stop inside an update of
  the window's content leaves the update queue of Vue unfinished and the
  window never updates again, with the error reported only. A memory or
  stack error is catchable and reaches the reporter, and nothing else
  happens. Fix: decide a default time, a setting that turns the limit on,
  and what the host does after a stop: show the error in the window, ask to
  restart, or quit with a status.

- **B-097 `console` prints some classes and built-ins unlike Node**
  `Units: jsenv · Size: M · Impact: Medium`

  Properties keyed by a `Symbol` and extra properties on an array, a typed
  array or an `Error` are dropped. A `WeakMap` and a boxed primitive print
  as a plain object, `{}`, where Node prints `WeakMap { <items unknown> }`
  and `[Number: 1]`. `%o` leaves out the hidden fields of a typed array. A
  nested string, and an object key that is not an identifier, is always
  single-quoted, where Node picks the quote that avoids escaping.

- **B-122 Colors take fewer notations than CSS**
  `Units: gpui,docs · Size: M · Impact: Medium`

  A color is a `0xRRGGBB` number, a hex string of 3, 4, 6 or 8 digits,
  `transparent`, `rgb()` or `rgba()`. CSS adds, from small to large:
  `currentcolor`, `hsl()` and `hsla()` in the comma and the space syntax with
  an alpha, the 148 color names, `hwb()`, `lab()`, `lch()`, `oklab()`,
  `oklch()`, `color()`, `color-mix()`, relative colors, `light-dark()`, system
  colors, `none` and `calc()` inside `rgb()`, and `rgb(10%20%30%)` without
  spaces. `var(--x)` waits for CSS variables. A parsing crate such as
  `csscolorparser` covers part of the list, and its range needs a check before
  use. Only `background`, `border_color` and `text_color` take a color.
  `outline-color`, per-side border colors, `box-shadow` and
  `text-decoration-color` come with the properties they belong to.

- **B-126 No test resolves `incajs/vue/runtime` through the published
  `exports`**
  `Units: cli,core · Size: S · Impact: Low`

  Compiled templates import `incajs/vue/runtime`, which re-exports from
  `@vue/runtime-dom`, an optional peer of `incajs` like `@vue/runtime-core`. No test installs the packed
  `incajs` into a scratch app and resolves `incajs/vue/runtime` through the
  published `exports`. The UI tests cover the workspace link.

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
  - A key the input method takes during a composition fires no `keydown` or
    `keyup`, where browsers fire `keydown` with `keyCode` 229. The fix
    direction is a flag from `gpui` that marks a key the input method
    consumed, so the host fires the events with `isComposing` and 229.
  - Blocked on `gpui` itself: `code` and `location` — `Keystroke`
    (`third_party/zed/crates/gpui/src/platform/keystroke.rs`) carries
    only `{ modifiers, key, key_char }`, with no layout-independent
    scancode and no left/right or numpad distinction to recover either
    from. `location` is 0 for every key and `code` is an empty string. The
    fix direction is a `gpui` platform change that reports the scancode and
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

- **B-146 Pointer fields of `click` follow Chromium only**
  `Units: bridge,docs · Size: S · Impact: Low`

  `click`, `auxclick` and `contextmenu` report `isPrimary` false. WebKit and
  Firefox report `true`, Firefox reports `pointerId` 0 and
  WebKit puts the click of a key at the center of the element and `pressure`
  0.5 on `contextmenu`. Fix: switch `isPrimary` to `true` and add the
  keyboard-click center once Chromium does.

- **B-148 `pointercancel` is never dispatched**
  `Units: gpui,bridge,docs · Size: S · Impact: Low`

  The host dispatches no `pointercancel`. Browsers fire it when a drag starts
  (Pointer Events, "suppress a pointer event stream"). The window losing focus
  during a press is a second trigger. Fix: dispatch it when B-155 starts a
  drag and when a press is reset.

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

  `width`, `height`, `min_*` and `max_*` take px numbers or px strings (or
  `"auto"`) only, and `padding` and `margin` take px numbers or px strings
  (`margin` also `"auto"`), so percentages are ignored, and `flex_basis` is not
  a key. `padding` and `margin` read one value per key, so the CSS shorthand
  `"1 2 3 4"` is ignored. The length keys `width`, `height`, `min_*` and
  `max_*` accept `NaN` and infinity.

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
  CLI's `optionalDependencies`, `host/package.mts`, `cd.yml` (the matrix, the
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
  host dispatches a named event at `window` through the existing
  `addEventListener` path. The hook is observe-only: when a reload's hook
  would fire, the new bundle has already loaded, so there is nothing left to
  veto. A handler can await only microtasks that have already settled,
  because the job queue drains once after the hook and the reload stops the
  timers of the old session, and QuickJS has no I/O to resume it.
  `packages/core` wraps the event names in helpers, so the strings never
  become app-facing API.

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

- **B-129 A focused node looks like an unfocused one**
  `Units: gpui,core · Size: L · Impact: Low`

  A focused node has no visual sign. The fix adds a
  `:focus` state to the host's style model: a style variant that applies while
  the node holds focus, set from the same focus transitions that dispatch
  `focus` and `blur`.

- **B-131 `:focus` is unsupported**
  `Units: gpui,core · Size: S · Impact: Low`

  Fix: add `:focus` with the interactive visual state.

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
  the rule that JS does not run per frame.

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

- **B-168 The `signal` option of `addEventListener()` is unsupported**
  `Units: core,bridge · Size: M · Impact: Low`

  A `signal` option is ignored, so an aborted signal leaves the listener
  registered. Fix: add `AbortController` to the
  runtime, then drop the registration when the signal aborts.

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

- **B-174 `behavior: "smooth"` scrolls jump**
  `Units: gpui,bridge,core · Size: M · Impact: Low`

  `scrollTo`, `scrollBy` and `scrollIntoView` apply every scroll at once. A
  browser animates a `smooth` scroll.

  Fix: an animation the host drives per frame, because the JS runtime does not
  run per frame.

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

- **B-198 Chromium fires `blur` and `focusout` when a focused node is removed or moved**
  `Units: bridge,docs · Size: S · Impact: Low`

  Chromium 153 fires `blur` and `focusout` at the removed or moved node, at the
  call. Firefox 155 and WebKit 26.6 fire neither. The host drops the focus at
  the call and fires neither, like Firefox and WebKit. Measured in
  `tmp/browser-check/focus-detach-results.md`. A fix fires the pair at the call
  and needs a decision on the engine to follow.

- **B-199 `target` is the mount container where browsers report `body`**
  `Units: gpui,bridge,core,docs · Size: M–L · Impact: Low`

  A key event while nothing is focused has the mount container as `target`,
  and its `composedPath()` is `[container, document, window]`. Browsers
  report `body` as `target`, with `html`, `document` and `window` above it.

  A mouse press or click outside every element has the container as
  `target`. Chromium and WebKit report `body`, and Firefox reports `html`.
  Moving the pointer from an element to an empty area fires `mouseout` and
  `mouseleave` with the container as `relatedTarget`, and `mouseover` at the
  container. Chromium and WebKit give `body` in both places, and Firefox
  gives `html`.

  The container stands in for `html` and `body`, and is the only node above
  the app. Fix: a host-owned body node under the root that the app mounts
  into and that takes these events.

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

- **B-206 `inca.config.ts` cannot configure Vue's compiler features**
  `Units: cli,docs · Size: M · Impact: Low`

  `@vitejs/plugin-vue` takes `features` options such as `optionsAPI`,
  `prodDevtools` and `prodHydrationMismatchDetails`, and Vite takes
  `define`. The CLI runs Vite with `configFile: false`, and `IncaConfig` has
  no field for either, so an app cannot turn the Options API off or set its
  own compile-time flags. B-104 covers choosing a bundler adapter. Fix: an
  `inca.config.ts` field that the Vite adapter passes to `vue()` and
  `define`.

- **B-207 An event handler attribute runs nothing**
  `Units: core · Size: L · Impact: Low`

  An attribute that holds code, such as `onclick="count++"`, stays a plain
  attribute and runs nothing, where a browser compiles it into the element's
  handler. Fix: needs a decision on running code from strings.

- **B-211 A lone CR inside a block comment shifts mapped lines**
  `Units: jsenv · Size: M · Impact: Low`

  `QuickJS` counts `\n` and `\r\n` inside a block comment as a new line, and a
  lone `\r` there as none (`quickjs.c:23133-23143`), while the remap counts a
  lone `\r` everywhere, as source map generators do. A frame below such a
  comment maps one line too far.

  Fix: count lines as `QuickJS` does, which needs the remap to track block
  comments in the generated source.

- **B-212 Some `sourceMappingURL` forms are not decoded**
  `Units: jsenv · Size: S · Impact: Low`

  A `data:` URL without `;base64` (percent-encoded JSON) and a sidecar name with
  `%`-escapes are read as written, so their maps are not found. Browsers decode
  both.

  Fix: percent-decode the URL, and accept a `data:` URL without `;base64`.

- **B-214 `console.log` of an event shows only `isTrusted`**
  `Units: jsenv,bridge · Size: M · Impact: Low`

  The console prints own properties and calls no getter, and an event owns
  only `isTrusted`, so `console.log(event)` prints
  `PointerEvent { isTrusted: [Getter] }` for a click. Node prints `type`,
  `defaultPrevented`, `cancelable` and `timeStamp`, and browser consoles list
  the getters. Fix: let the host name, per prototype, the getters the console
  reads, and have the event prototypes name their base fields.

- **B-215 The scroll range relies on an anchor child**
  `Units: gpui · Size: S · Impact: Low`

  Every scroll container carries a zero-size, absolutely placed child before
  its children, so that the `gpui` scroll range covers the padding box. `gpui`
  subtracts the border box where the client size is the padding box, and
  measures from the first child box where the range starts at the content
  box. The anchor costs one `gpui` element and one layout node per scroll
  container, and `record_children` skips it by counting the children from the
  end. It has no hitbox and no listener, and the virtual tree, the snapshot
  and the `children` of a template `ref` do not list it.

  Fix: change the range computation in `gpui` (`div.rs`,
  `clamp_scroll_position` and the content size of the children) and drop the
  anchor once the host can patch `gpui` or an upstream release carries the
  change.

- **B-216 A child's end margin is outside the scroll range**
  `Units: gpui · Size: M · Impact: Low`

  At the maximum offset the child of a scroll container that ends farthest
  meets the end padding with its border box, and `scrollHeight` and
  `scrollWidth` leave its `margin-bottom` and `margin-right` out. Browsers
  include the margin.

  Fix: `gpui` measures the range over the border boxes of the children. The
  range needs the end margins added, for example through a `gpui` change that
  measures margin boxes.

- **B-217 Content that overflows a child is outside the scroll range**
  `Units: gpui · Size: L · Impact: Low`

  A child of a scroll container that is smaller than its own content, with
  `overflow` `visible`, adds only its own box to the range. Browsers add the
  overflowing content.

  Fix: `gpui` measures the direct children only. The range needs the overflow
  of the descendants, computed in the host from the recorded boxes or in
  `gpui`.

- **B-218 The commands resolve the app config through two paths that check different things**
  `Units: cli · Size: M · Impact: Low`

  `inca package` calls `resolveAppConfig`
  (`packages/cli/src/package.mts`). It requires `package.json`, requires a
  usable product name, refuses an `icon` that does not exist and refuses a
  bad `outDir`. `inca build` and `inca config` call `resolveRuntimeConfig`,
  which reads `package.json` only if it is there, checks the product name
  only when one is set and checks neither the icon nor `outDir`; `inca build`
  checks `outDir` through `resolveBuildConfig` and `assertOutDir`. `inca dev`
  runs `resolveBuildConfig` and `assertOutDir`, then `resolveAppConfig`, and
  on a missing `package.json` or name prints `running unnamed. …` to stdout
  and falls back to `resolveRuntimeConfig`. Any other failure there, such as
  a missing icon, stops `inca dev` where `inca build` succeeds. Each
  resolver loads the config file again, up to three loads per `inca dev`
  attempt.

  Fix: one resolver that loads the config once and runs the checks as
  separate steps, each command choosing the steps it needs.

- **B-220 The CLI spells the config file name twice**
  `Units: cli · Size: S · Impact: Low`

  The Vite adapter and `inca config` each write `inca.json` by name.

  Fix: inject the name through the adapter core, which leaves one spelling.

- **B-221 Events built from script miss some members of their class**
  `Units: bridge,host,core,docs · Size: M · Impact: Low`

  `initUIEvent()`, `initMouseEvent()`, `initKeyboardEvent()` and
  `initCompositionEvent()` are missing. An init dictionary's `view` is not read,
  `relatedTarget` takes any object, and `modifierAltGraph`, `modifierCapsLock`
  and the other `modifier` members are ignored. A built mouse event keeps
  `offsetX` and `offsetY` equal to its client position while it is dispatched;
  browsers measure them from the target's padding edge. `PointerEvent` has no
  `tiltX`, `tiltY`, `twist`, `tangentialPressure`, `altitudeAngle`,
  `azimuthAngle`, `persistentDeviceId`, `getCoalescedEvents()` or
  `getPredictedEvents()`. Every UI event reads `view` as `null`, where browsers
  give `window`. The `dataTransfer` and `targetRanges` members of an
  `InputEventInit` are not read.

  Fix: add the legacy init methods, keep the modifier states in the
  payload and set the offsets from the target's box in `dispatchEvent`. The
  host already exposes `window` and `document`, so `view` can name the
  window. A brand check on `relatedTarget`, so that only elements, `document`
  and the window are accepted, is the stricter option.

- **B-222 A template handler has no event type**
  `Units: core,cli,docs · Size: M · Impact: Low`

  `addEventListener` and the `on<event>` properties take their event type
  from `IncaEventMap`. A handler in a template, such as
  `@click="(e) => ..."`, is typed by vue-tsc from the element declarations
  of the DOM, which the repository does not provide. Fix: element type
  declarations for vue-tsc, or types generated into `.inca/`, as Nuxt does
  into `.nuxt/`, so that a wrong annotation is an error; that needs a
  stricter constraint on the generic listener type.

- **B-223 `keyCode` follows a US keyboard layout**
  `Units: gpui,bridge · Size: S–M · Impact: Low`

  `keyCode` and `which` of a key event come from `key` through a US-layout
  table, so `é` gives 0 and a key typed on another layout gives the US code
  of its character. `code` is an empty string, because the physical key
  position is unsupported. Fix: carry gpui's `Keystroke::key`, the ASCII
  character printed on the key, in the key payload and map that.

- **B-224 Files in `public/` are not part of a packaged app**
  `Units: cli · Size: S · Impact: Low`

  `inca build` copies the files of `public/` into the output directory, and
  `inca package` copies only the files the build emitted, so they are not in
  the packaged app.

- **B-225 Source maps of old compiles are never dropped**
  `Units: jsenv · Size: M · Impact: Low`

  Each compile keeps its generated source and map for the life of the
  context. A long `inca dev` session that edits a module many times keeps every
  version. A function of an old module can outlive the function that compiled
  it, so the host cannot tell when a version is unreachable.

  Fix: keep the newest versions of each file name and drop older ones, or have
  the evaluator hold each module's compiled function for as long as the module
  stays cached, then drop the entry with the function.

- **B-226 `window` and `document` need an import**
  `Units: cli,core,docs · Size: M · Impact: Low`

  `window` and `document` are exports of `incajs`, and `globalThis.window` and
  `globalThis.document` are undefined. Defining them breaks `vue`'s runtime
  DOM, which calls `document.createElement` at load when `document` exists.
  Fix: a Vite plugin injects `import { document, window } from "incajs"` into
  the app's own source, not into `node_modules`, and generates their types
  into `.inca/`.

- **B-228 The text input lacks commands and members of a browser text field**
  `Units: gpui,bridge,core,docs · Size: L · Impact: Low`

  An `input` moves its caret by grapheme and by the whole line only.
  Word-wise moves and deletes (Control or Alt with an arrow or Backspace),
  the macOS shortcuts (Cmd+Left, Cmd+Right, Cmd+Backspace, Option with an
  arrow, Ctrl+A and Ctrl+E), undo and redo, double- and triple-click
  selection, text drag and drop and a blinking caret are missing. Every
  `type` renders as plain text, so a password shows its characters. `select`
  and `selectionchange` are never dispatched. `selectionStart`,
  `selectionEnd`, `selectionDirection`, `setSelectionRange()`, `select()`,
  `defaultValue`, `maxLength`, `size` and `textarea` are missing. Fix: word
  boundaries from `unicode-segmentation`, an edit history with `historyUndo`
  and `historyRedo`, the selection members over the host's selection, and a
  masked display for `type="password"`.

- **B-229 `copy`, `cut` and `paste` are never dispatched**
  `Units: gpui,bridge,core,docs · Size: M · Impact: Low`

  The clipboard shortcuts of an `input` read and write the clipboard
  directly, so a `copy`, `cut` or `paste` listener has no effect and cannot
  change or cancel the transfer. Fix: fire a `ClipboardEvent` whose
  `clipboardData` holds the text before the transfer, and skip the transfer
  when a listener calls `preventDefault()`.

- **B-231 A built app fails to load an entry whose top-level `await` is still pending**
  `Units: host · Size: M · Impact: Low`

  `Session::load_with` (`crates/inca-host/src/app.rs`) evaluates the entry with
  `eval_module` before the window opens, and `eval_module` fails with an
  unsettled-promise error when a top-level `await` is still pending once the
  queued jobs have run. `inca dev` runs the same entry, because the serve loop
  keeps advancing a loading session. An entry that awaits a promise settled
  later, such as a timer, loads in development and fails in a built app.
  Found by reading; not reproduced. Fix: start a built app through
  `Session::begin_with` and the serve loop, which also gives it the load wait of
  `inca dev`.

- **B-232 `setTimeout` and `setInterval` refuse a string handler**
  `Units: core,docs · Size: S–M · Impact: Low`

  A handler that is not a function throws a `TypeError`. Browsers convert it
  to a string and run it as global code when the timer fires, unless a content
  security policy forbids it. Choices: accept every string as global indirect
  `eval`, accept only a `TrustedScript`, or keep the `TypeError`. Accepting a
  string opens a path that runs text as code, and the host has no policy to
  limit it, so the choice waits for a decision on Trusted Types and `eval`.

- **B-233 A global that `incajs` defines is missing until `incajs` loads**
  `Units: core,cli,docs · Size: S–M · Impact: Low`

  Core defines the timer functions when the `incajs` or `incajs/vue` module
  loads. A module that runs earlier and calls one of them at its top level
  fails with `ReferenceError`. The entry that `inca` generates imports
  `incajs/vue` before `App`, so a `.vue` file is safe there. A hand-written
  entry that imports `App` first is not. Fix: the injection of B-226 adds the
  import for the identifiers that core defines. A bare `import "incajs"`
  does not do it: the bundler drops `incajs` when the app uses no export of
  it.

- **B-236 Nested components hit the stack limit at a smaller depth than in browsers**
  `Units: jsenv,host · Size: L · Impact: Low`

  At the 6 MiB stack limit a tree of nested components reaches 493 levels,
  349 for a component in a `div`, and 215 for slot wrappers. Chromium reaches
  750, 562 and 466, Firefox 1498, 1094 and 701, and WebKit 3001, 1716 and
  1164. Matching Chromium needs about 13 MiB, and the main thread of Linux
  has 8 MiB by default. Fix: run the engine on a thread with a larger stack,
  or cut the native frames per call.

- **B-237 Keywords in `:style` are lowercase only**
  `Units: gpui,docs · Size: S · Impact: Low`

  CSS keywords are ASCII case-insensitive, and browsers accept `ROW-REVERSE`.
  The host reads a color keyword in any case and ignores any other keyword
  that is not lowercase, with a warning. Fix: compare those keywords
  case-insensitively in the style parser (`keywords!` in
  `crates/inca-gpui/src/element/spec.rs`) and document the rule.

- **B-238 `keypress` goes to the node its `keydown` went to**
  `Units: bridge · Size: M · Impact: Low`

  A `keydown` listener that moves the focus or removes the focused node sends
  the `keypress` of the same press, and the typed text, to the node the focus
  left. Chromium, Firefox and WebKit send them to the new focus, or to `body`
  when the node is gone. The host applies a focus request at the next frame,
  and a node that Vue removes stays detached until a frame frees it, so the
  detached node still hears the `keypress`. Fix: apply the focus inside the
  key path before `keypress` fires, then recheck the target.

- **B-239 `keypress` fires for a dead key on macOS**
  `Units: gpui,bridge · Size: S–M · Impact: Low`

  Option+E on a US Mac layout is a dead key. gpui reports the accent as
  `key_char`, so the host fires `keypress` with `key` `´`, and `keydown` reads
  `´` as `key`. Chrome fires `keydown` with `key` `Dead` and `keyCode` 229 and
  no `keypress`. Fix: report a dead key as `Dead` from the platform layer, or
  skip `keypress` for a key that starts a composition.

- **B-240 A module the time limit stops loses the limit in its error**
  `Units: jsenv · Size: S–M · Impact: Low`

  A module that the time limit stops in `start_module` reads as
  `InternalError: interrupted`, without the limit, when another call into the
  engine runs before `poll_module` reads it. The engine remembers a stop for
  one call, and each call forgets the stop of the call before. The host polls
  right after it starts the entry, and its time limit is off, so it does not
  reach this. Fix: record the stop with the module.

- **B-241 `panic.log` loses earlier panics and can be written through a link**
  `Units: host · Size: S–M · Impact: Low`

  The next panic replaces `panic.log`. When the host finds no home directory,
  it writes below the temporary directory, where another user can place a link
  named `panic.log` before the first panic. Fix: keep each panic in a new file
  that is created exclusively, in a private directory owned by the user.
  Another option is to let the person choose the save location in an OS dialog
  when the app crashes.

- **B-242 The host binary can't unpack itself**
  `Units: host,ci,cli · Size: L · Impact: Low`

  The host binary ships compressed in its npm package and the CLI unpacks it.
  An app that `inca package` builds holds the unpacked binary, so the app is as
  large as the binary. Fix: make `inca-host` a small launcher that holds the
  compressed binary, unpacks it into a cache directory on the first start,
  checks it and replaces itself with it. On Linux, an anonymous in-memory file
  could skip the cache at the cost of unpacking on every start. On macOS the
  unpacked binary needs its own signature and lives outside the `.app`, so it
  needs a way to find the bundle.

- **B-243 The CLI has no rule for which stream a line goes to**
  `Units: cli · Size: S · Impact: Low`

  `build`, `package` and `dev` write their progress and result lines to
  stdout, while a warning and the shutdown line go to stderr. The line that
  names the directory of a first unpack goes to stderr, because `inca config`
  prints JSON on stdout. Nothing records which kind of line uses which stream.
  Fix: decide one rule, apply it to every `log` and `warn` call and write it in
  `handbook/STYLE.md`.
