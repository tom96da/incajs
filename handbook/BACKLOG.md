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
  `Units: host · Size: S · Impact: Medium`

  `inca dev` reports a host panic as a `-32603` error. A packaged app has no
  stdout client and its stderr goes nowhere, so a panic there leaves nothing
  behind. It needs a log file or a dialog. In `inca dev` a panic shows twice:
  once from the default hook on stderr, and once as the `-32603` line the CLI
  prints. A host thread blocked writing to a stalled stdout pipe holds the
  hook, though the panic text has already reached stderr. It is the first
  half of the crash reporting in
  [ROADMAP.md](./ROADMAP.md#known-gaps-not-yet-scheduled); the destination is
  undecided.

## P2

- **B-007 `.vue` `<style>` blocks don't reach the screen**
  `Units: cli,core,gpui · Size: L · Impact: Medium`

  Scoped/global
  `<style>` blocks in `.vue` SFCs have no effect through the custom
  renderer. A build now fails on one rather than shipping it
  (`ERR_INCA_UNSUPPORTED_STYLE`, raised by `adapter/vite/unsupported.mts`);
  rendering them is Phase 7's work.

- **B-012 The window is set once and never follows the app**
  `Units: bridge,host,core · Size: L · Impact: Medium`

  `start()`
  (`crates/inca-host/src/app.rs`) reads the size and title out of the
  config and the mounted root, hands them to `WindowOptions`, and nothing
  revisits them. An app that changes its root's `width`/`height`, or wants
  a title that tracks its state, has no way to move the window. GPUI has
  `Window::resize` and `Window::set_window_title`
  (`third_party/zed/crates/gpui/src/window.rs:2622, 2732`); reaching them
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

- **B-018 `event.target` is an approximation**
  `Units: bridge,gpui · Size: L · Impact: Medium`

  GPUI only gives a container a
  hitbox when something listens on it, so a click on a listener-less child
  can't be traced to that child — `EventDispatcher::dispatch`
  (`crates/inca-bridge/src/dispatch.rs`) sets `target` to the same node as
  `currentTarget`, the node the call is dispatching for. Making it exact
  means giving every container a hitbox while any node in the tree has a
  mouse listener, then reading the innermost one hit_test finds. That
  costs a hitbox and a no-op listener call per node per pointer event.
  The field name doesn't need to change for this to land later; only
  its accuracy would improve. Until it does, nothing resembling event
  delegation (a handler relying on which descendant was actually hit)
  can be written against this framework.

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

  Three separate
  gaps now live loose in `dispatch.rs` — B-018 and B-019 plus
  `mouseenter` firing on mount for an element already under the pointer
  (GPUI's hover check compares against freshly-initialized state on first
  paint, not against a real pointer move; see `docs/reference/events.md`). `EventDispatcher` exists to turn GPUI's raw input
  into what a DOM author expects, so this translation belongs there too.
  Direction: one `compat` submodule inside it, not a separate crate — one
  place to hold `held_buttons` tracking, a "no real pointer move seen
  yet" flag suppressing the mount-time `mouseenter`, and later the
  capture-phase (B-019) and precise-`target` (B-018) work.

- **B-022 No Tab-key focus navigation**
  `Units: bridge,gpui,core · Size: M–L · Impact: Medium`

  `crates/inca-bridge/src/focus.rs` only
  moves focus on an explicit `focusNode` call or a click landing on a
  focusable, `.id()`-bearing node. GPUI already has `tab_index`/
  `tab_stop`/`window.focus_next(cx)` for this — wiring it means deciding
  what `inca`'s tab-order vocabulary looks like (a style prop? an
  attribute?) and is a separate unit from the focus model itself.

- **B-024 Destroying a focused node must come to fire `"blur"`**
  `Units: bridge,core · Size: L · Impact: Medium`

  It doesn't
  today — `destroyNode`'s JS wrapper (`packages/core/src/tree.mts`) drops
  every freed callback synchronously (`releaseCallbacks`), so even a
  deferred, next-frame `"blur"` dispatch would find nothing left to call.
  Closing this needs one of: delaying a focused node's own callback
  release by one frame (a `destroyNode` contract change — every other
  node's callbacks still free immediately), or giving native bindings
  synchronous `Window`/`App` access so `destroyNode` itself can dispatch
  `"blur"` before anything is freed (a wider change to the binding/
  dispatch boundary every `crate::bindings` function currently shares).
  Land it alongside whatever else motivates that wider change.

- **B-025 `"keydown"`/`"keyup"` reach no node with nothing focused**
  `Units: gpui,host · Size: M · Impact: Medium`

  The DOM
  falls back to `document.body` as the target; `inca` has no such
  fallback, so a key press before anything is focused is silently
  dropped. `key` can also stay lowercase for a shifted letter on a
  platform whose `Keystroke::key_char` doesn't report the shifted
  character.

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

- **B-034 A stale `node_modules/.inca/hmr/inca.json` outlives the config that wrote it**
  `Units: cli · Size: S · Impact: Medium`

  `hmr()` (`packages/cli/src/adapter/vite/hmr.mts`) only writes
  the file when `serializeConfig(runtimeConfig)` returns something; if a
  later session's config no longer produces any content, the old file is
  never removed, so a stale `window`/etc. section keeps being read.

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

- **B-041 A camelCase (or otherwise miscased) style key is silently ignored**
  `Units: gpui · Size: S · Impact: Medium`

  `style_spec_from_props` (`crates/inca-gpui/src/element.rs`)'s match falls
  through to `_ => {}` for any key it doesn't recognize, including
  `flexDirection` where `flex_direction` was meant — no error, no warning,
  the property just never applies. Someone coming from web-standard CSS/
  Vue conventions hits this as a silent no-op with nothing to point at the
  typo. At minimum this should warn in dev mode. Keys such as `padding_top` are
  snake_case too, so `paddingTop` is ignored the same way.

- **B-045 `buttons` stays set after a release nobody listens to**
  `Units: bridge · Size: M · Impact: Medium`

  `update_held_buttons` (`crates/inca-bridge/src/dispatch.rs`) only runs
  when a listener is dispatched. A node that listens for `mousedown` and
  `mousemove` but not `mouseup` never clears the pressed bit, so later
  `mousemove` events report the button as held. Fix: track buttons from the
  raw window event. This belongs in the `compat` submodule of B-020.

- **B-046 `click` carries no modifier or button fields**
  `Units: gpui,bridge,docs · Size: M · Impact: Medium`

  `click` has
  `EventPayload::None`, so Vue's `@click.ctrl`/`.shift`/`.alt`/`.meta`
  checks see `undefined` and never run the handler. `@click.right` compiles
  to a `contextmenu` listener that never fires. `docs/reference/events.md`'s
  "Event modifiers" section names neither limit.

- **B-047 A non-style prop can't be removed once set**
  `Units: gpui,bridge,core · Size: M · Impact: Medium`

  `patchProp` passes a
  string, number, or boolean `nextValue` to `core.setAttribute` and ignores
  anything else, and there is no native `removeAttribute`. A prop that
  disappears from a vnode, or turns `null`/`undefined`, keeps its last
  value in the node's `attributes` map. Fixing it needs a `remove_attribute`
  on the tree, a matching bridge binding, and `patchProp` calling it.

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

  those listeners run inside `render()`, where GPUI ignores
  `window.refresh()` (`third_party/zed/crates/gpui/src/window.rs:2178`), so
  the request may wait for an unrelated redraw. Unconfirmed.

- **B-059 No engine limits or interrupt handler**
  `Units: jsenv,host · Size: M · Impact: Medium`

  `EngineBuilder`
  (`crates/inca-jsenv/src/engine.rs`) sets no memory limit, stack size, or
  interrupt handler, so a runaway app can't be stopped. rquickjs offers
  `Runtime::set_memory_limit`/`set_max_stack_size`. Defaults and how the
  host reports a timeout are undecided.

- **B-063 `withModifiers` and `withKeys` are not exported to compiled templates**
  `Units: cli,core · Size: M · Impact: Medium`

  `withModifiers` and `withKeys` are not exported by the runtime the compiled
  templates import from. `@click.stop`, `.prevent`, `.ctrl` and key modifiers
  such as `@keydown.enter` fail the build. Related: B-046 (click carries no
  modifier fields) assumes these compile.

- **B-068 `click` fires before `mouseup`**
  `Units: gpui · Size: M · Impact: Medium`

  `click` fires before `mouseup` on the same node. `stopPropagation()` in a
  `click` handler also stops ancestors' `mouseup`. The DOM order is `mouseup`
  then `click`.

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

  A class instance prints without its class name, as `{ a: 1 }` where Node
  prints `Foo { a: 1 }`. Properties keyed by a `Symbol` and extra
  properties on an array or an `Error` are dropped. A `Promise`, a typed
  array, a `WeakMap` and a boxed primitive print as a plain object: `{}` for
  most, `{ '0': 0, '1': 0, '2': 0 }` for `new Uint8Array(3)`, where Node
  prints `Promise { 1 }` and `Uint8Array(3) [ 0, 0, 0 ]`. A nested string,
  and an object key that is not an identifier, is always single-quoted,
  where Node picks the quote that avoids escaping.

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

- **B-009 CI's path filter skips JS jobs for a `tests/`-only change**
  `Units: ci · Size: S · Impact: Low`

  `.github/workflows/ci.yml`'s `changed` job lists `tests/**` only under
  its `rust` filter, not `js`, even though `tests/` also holds `.mts`
  tests. A PR touching only those runs no lint/format/typecheck job.

- **B-010 `packages/cli`'s `sideEffects` list omits `dist/hmr-runtime.js`**
  `Units: cli · Size: S · Impact: Low`

  only `src/adapter/vite/runtime/globals.mts` is declared, but the built
  runtime installs globals the same way. A bundler that tree-shakes on
  `sideEffects` could drop it.

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

- **B-015 A `--print-config` diagnostic**
  `Units: host · Size: S · Impact: Low`

  Nothing shows which settings an app is
  actually running under. The host reads `inca.json` beside the entry and
  applies what it can use; a flag that read it and printed the result would
  answer "why is my window that size" without a build. Running it from the
  CLI to validate would make `inca build` need a host binary, which it
  doesn't today — so this is a diagnostic, not a build step.

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

- **B-021 `"focus"`/`"blur"` will need to become `"focusin"`/`"focusout"` once focusable nodes can nest**
  `Units: bridge · Size: S–M · Impact: Low`

  `crates/inca-bridge/src/focus.rs`'s
  `FocusRegistry` dispatches DOM's non-bubbling `focus`/`blur` today,
  correct only because no focusable node has a focusable descendant yet.
  A focusable container wrapping a focusable child would need the
  bubbling pair instead — checking whether the node whose focus state
  changed is the exact one focused, not just an ancestor of it.

- **B-023 No way to make a node focusable without also focusing it**
  `Units: bridge,core · Size: M · Impact: Low`

  `FocusRegistry::get_or_create` (`crates/inca-bridge/src/focus.rs`) is
  the only place a `FocusHandle` gets allocated, and it only ever runs
  from `apply_pending`'s `PendingFocus::Focus` branch, which calls
  `handle.focus(window, cx)` immediately afterward — `focusNode`
  conflates "become focusable" with "focus now." A future `input`/
  `button`-like element that should already be click-focusable when it
  mounts, the way a real `<input>` is with no script ever calling
  `.focus()`, needs a binding that only allocates the handle and wires
  `.track_focus` (`crates/inca-gpui/src/element.rs`'s `wire_focus`),
  without moving focus. Separate from Tab-key navigation (B-022) — that's
  about keyboard order once a node is already focusable, this is about
  becoming focusable at all.

- **B-026 More `MouseEvent`/`KeyboardEvent` fields could reach JS**
  `Units: gpui,bridge · Size: M–L · Impact: Low`

  `crates/inca-gpui/src/event_sink.rs`'s `MousePayload`/`KeyPayload`
  carry a deliberate subset of each DOM type's own fields. What's between
  each missing one and landing differs:
  - Computable today, but not a small change — no `gpui` change needed,
    but its own state has to be threaded from element-build time into
    dispatch, and `gpui` has no production API to look a node's bounds
    up by id at event time otherwise: `offsetX`/`offsetY` (a target's
    own bounds are already computed once, for hitbox insertion, just
    never kept anywhere `dispatch` can reach).
  - Blocked on another `inca` gap, not on `gpui`: `relatedTarget` needs
    the same precise hit-testing the `event.target` approximation (B-018)
    is waiting on; `isComposing` needs the text-editing/IME unit,
    which hasn't started.
  - Blocked on `gpui` itself: `code` and `location` — `Keystroke`
    (`third_party/zed/crates/gpui/src/platform/keystroke.rs`) carries
    only `{ modifiers, key, key_char }`, with no layout-independent
    scancode and no left/right or numpad distinction to recover either
    from.

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

- **B-039 A root that declares one dimension never auto-resizes the window**
  `Units: host · Size: S · Impact: Low`

  `maybe_auto_resize_to_content` (`crates/inca-host/src/app.rs`) needs
  both dimensions ready together. A root that only ever sets `width` or
  `height` (by design, not a timing accident) never triggers it. Resizing
  the one known axis would fix it.

- **B-040 `-32601` still doesn't return the method name**
  `Units: host · Size: S · Impact: Low`

  `handle_unrecognized`
  (`crates/inca-host/src/dev.rs`) has the unrecognized `method` in hand but
  answers a fixed `"unknown method"` string regardless. A client can't tell
  from its own log which method got rejected.

- **B-042 `resolveEntry` only recognizes `src/main.mts`, not `.ts`/`.js`**
  `Units: cli,docs · Size: S · Impact: Low`

  `packages/cli/src/entry.mts` checks the literal `src/main.mts` path and
  falls back to synthesizing an entry from `src/App.vue`; an `.mts`-less
  `src/main.ts`/`src/main.js` is invisible to it. `docs/reference/elements.md`
  already tells readers this is "intended for later, not yet implemented."
  Track the actual work here: accept `.ts`/`.js` too, or write down why
  `.mts` alone is required.

- **B-049 Colour parsing is looser than documented**
  `Units: gpui,docs · Size: S · Impact: Low`

  `parse_hex_color`
  (`crates/inca-gpui/src/element.rs`) accepts `"#+abcde"` because Rust's
  hex parser allows a leading `+`. Numeric colours are cast with `as u32`,
  so a negative or `NaN` value becomes black and a value above `0xFFFFFF`
  loses its top byte. `docs/reference/elements.md` describes neither.

- **B-052 A malformed `package.json` fails inconsistently**
  `Units: cli · Size: S · Impact: Low`

  `resolveAppConfig`
  (`packages/cli/src/config/loader.mts`) lets the raw `JSON.parse` error
  escape with no `ERR_INCA_*` code. `resolveRuntimeConfig` swallows the same
  error and runs unnamed.

- **B-055 Two small rule mismatches**
  `Units: gpui,jsenv · Size: S · Impact: Low`

  `VirtualTree::create_node`'s `.expect` on
  id exhaustion (`crates/inca-gpui/src/tree.rs`) contradicts
  `FAILURES.md`'s "doesn't panic" rule, though reaching it isn't
  practical. `console.log` has no cap on array length.

- **B-065 A throwing handler is reported twice in a production build**
  `Units: core · Size: S · Impact: Low`

  In a production build a throwing handler is reported twice. Vue's default
  error logging reports it once, and the wrapper's rethrow reports it again.

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

  A file that exists but is not executable passes `assertHostBin`, although
  the error message and `docs/reference/errors.md` say the binary must be
  executable. That case takes the mode-dependent path above.

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
  ignored. A negative `padding` is ignored without a warning. The length
  keys `width`, `height`, `min_*` and `max_*` accept `NaN` and infinity.

- **B-086 Some `console` methods only approximate the standard**
  `Units: jsenv · Size: M · Impact: Low`

  A format string supports `%s`, `%d`, `%i`, `%f`, `%o`, `%O`, `%c` and
  `%%`. `%o` and `%O` render alike, and `%c` drops its argument without
  styling. A conversion that throws prints the value as `dir` does, where
  the standard throws. A `Symbol` or a label whose `toString` throws becomes
  the default label where the standard throws. `time`, `timeLog` and
  `timeEnd` always print milliseconds. `dir` ignores its `options`. `dirxml`
  formats like `log`. `clear` closes the open groups and leaves the terminal
  as it is. `groupCollapsed` prints its lines like `group`, since a terminal
  cannot fold them. `table` ignores a `columns` argument that is not an
  array, and drops properties keyed by a `Symbol` and a `Symbol` column
  name. `trace` reads its stack through the global `Error`, which a script
  can replace. `profile`, `profileEnd` and `timeStamp` do not exist.

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
    where the standard wraps long output. The standard cuts a collection
    after 100 items (`... 50 more items`), where every entry is printed.
  - A `RegExp` has one colour, where the standard highlights its parts.
  - `%s`, `%d`, `%i` and `%f` convert an array that holds a `Proxy` through
    the array's own string form, which runs the `Proxy`'s traps.

- **B-090 Only the first error of several throwing handlers reaches the host's report**
  `Units: core · Size: S · Impact: Low`

  Handlers of one event (an array value, or `onClick` plus `onClickOnce`) run
  behind one host callback that rethrows the first error after all have run.
  The others go only to Vue's own error handler or the console, where the host
  reports every throwing callback.

- **B-091 A handler attached during an event can fire for that same event**
  `Units: core · Size: S · Impact: Low`

  Vue's DOM renderer drops a handler attached after the event began, using the
  `_vts` and `attached` timestamps in `runtime-dom`'s `events.ts`. `patchProp`
  has no such check, so a handler attached to an ancestor by the re-render
  that a child's handler triggers fires for the same event.

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
