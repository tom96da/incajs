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

- **The dev protocol always ships inside `inca-host`**: `crates/inca-host/src/dev.rs`
  (stdin/stdout JSON-RPC, the stdout writer thread, `reload`) compiles into
  every `inca-host` binary, including the ones `inca package` distributes,
  even though a packaged app never passes `--dev` and never runs any of it.
  Worth a way to drop it from a release build — a Cargo feature gating
  `mod dev;` and the CLI's `--dev` flag, built once for `inca dev`'s own use
  and once (without the feature) for packaging — once it's worth the two
  build configurations that implies.

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

- **CI's path filter skips JS jobs for a `tests/`-only change**:
  `.github/workflows/ci.yml`'s `changed` job lists `tests/**` only under
  its `rust` filter, not `js`, even though `tests/` also holds `.mts`
  tests. A PR touching only those runs no lint/format/typecheck job.

- **`packages/cli`'s `sideEffects` list omits `dist/hmr-runtime.js`**:
  only `src/adapter/vite/runtime/globals.mts` is declared, but the built
  runtime installs globals the same way. A bundler that tree-shakes on
  `sideEffects` could drop it.

- **`dev.mts` imports `vite`/`rolldown` directly, past the "only
  `adapter/vite` imports vite" rule**: `faultOf` (`packages/cli/src/dev.mts`)
  reaches for `buildErrorMessage`/`RollupError` outside the adapter layer
  `specs/ARCHITECTURE.md` says is the only place that imports `vite`. Move
  that formatting into `adapter/vite` and hand `dev.mts` an already-built
  `Fault`.

- **The window is set once and never follows the app**: `start()`
  (`crates/inca-host/src/app.rs`) reads the size and title out of the
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

- **`insert_before` with the anchor equal to the node being moved reorders
  it instead of leaving it alone**: `VirtualTree::insert_before`
  (`crates/inca-gpui/src/tree.rs`) detaches the child before resolving the
  anchor's position, so when the anchor is the child itself, the position
  lookup fails (the child isn't in the children list anymore) and the node
  falls through to being appended at the end. The DOM's own
  `insertBefore(node, node)` is a no-op; this moves it. `packages/core/src/vue/nodeOps.mts`
  passes the anchor straight through, so the same input pattern likely
  reaches this from the JS side too.

- **`event.target` is an approximation**: GPUI only gives a container a
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

- **No capture-phase listener registration**: GPUI's own mouse dispatch
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

- **A GPUI-vs-DOM compat layer for `crates/inca-bridge`**: three separate
  gaps now live loose in `dispatch.rs` — the two entries above plus
  `mouseenter` firing on mount for an element already under the pointer
  (GPUI's hover check compares against freshly-initialized state on first
  paint, not against a real pointer move; see `specs/FFI.md`'s "Event
  dispatch" section). `EventDispatcher` exists to turn GPUI's raw input
  into what a DOM author expects, so this translation belongs there too.
  Direction: one `compat` submodule inside it, not a separate crate — one
  place to hold `held_buttons` tracking, a "no real pointer move seen
  yet" flag suppressing the mount-time `mouseenter`, and later the
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

- **No way to make a node focusable without also focusing it**:
  `FocusRegistry::get_or_create` (`crates/inca-bridge/src/focus.rs`) is
  the only place a `FocusHandle` gets allocated, and it only ever runs
  from `apply_pending`'s `PendingFocus::Focus` branch, which calls
  `handle.focus(window, cx)` immediately afterward — `focusNode`
  conflates "become focusable" with "focus now." A future `input`/
  `button`-like element that should already be click-focusable when it
  mounts, the way a real `<input>` is with no script ever calling
  `.focus()`, needs a binding that only allocates the handle and wires
  `.track_focus` (`crates/inca-gpui/src/element.rs`'s `wire_focus`),
  without moving focus. Separate from Tab-key navigation above — that's
  about keyboard order once a node is already focusable, this is about
  becoming focusable at all.

- **Destroying a focused node must come to fire `"blur"`**: it doesn't
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

- **`"keydown"`/`"keyup"` reach no node with nothing focused**: the DOM
  falls back to `document.body` as the target; `inca` has no such
  fallback, so a key press before anything is focused is silently
  dropped. `key` can also stay lowercase for a shifted letter on a
  platform whose `Keystroke::key_char` doesn't report the shifted
  character.

- **More `MouseEvent`/`KeyboardEvent` fields could reach JS**:
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
    the same precise hit-testing the `event.target` approximation entry
    above is waiting on; `isComposing` needs the text-editing/IME unit,
    which hasn't started.
  - Blocked on `gpui` itself: `code` and `location` — `Keystroke`
    (`third_party/zed/crates/gpui/src/platform/keystroke.rs`) carries
    only `{ modifiers, key, key_char }`, with no layout-independent
    scancode and no left/right or numpad distinction to recover either
    from.

- **No input-synthesis/state-introspection channel on the dev protocol**:
  `tests/tests/hmr-quickjs-state.test.mts` drives its fixture with a
  `tick` dev notification standing in for a pointer click, because
  there's no way to synthesize one against a real running `inca-host`
  process from outside. A generalized version of that workaround — a dev
  protocol method for synthesizing input and reading back app state —
  would be the seed of future automated GUI testing (a Playwright-style
  tool driving the app), not just this one test's stimulus.

- **A template `ref` still resolves to a plain data object for anything
  beyond `.focus()`/`.blur()`**: `IncaElement`
  (`packages/core/src/vue/nodeOps.mts`) has those two methods; the same
  gap likely applies to any other DOM-element method or property a
  `.vue` app would otherwise reach for on a template `ref`.

- **`inca-host` doesn't exit after reporting a startup failure**: when the
  very first bundle fails to evaluate (before any window has opened),
  `report_startup_failure`/`cx.quit()` runs, but the process keeps
  running instead of actually terminating — reproduced by hand, `--dev`
  against a bundle that just throws, with the real compiled binary
  spawned as a child process (not `TestAppContext`, which never exercises
  real process exit). Suspected cause: `cx.quit()` only takes effect once
  the platform's native event loop has been kicked into motion by at
  least one window opening; with zero windows ever created, the quit
  request seems to go unprocessed. Affects any `inca dev` session (HMR or
  not) whose first bundle fails to load, not just experimental HMR.

- **`inca dev --experimental-hmr` opens its window at the wrong size on
  first launch, then resizes**: a plain `inca dev` opens already sized to
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

- **A style-only change to an already-mounted node doesn't reach the
  screen under `--experimental-hmr`, though the same edit works via a
  full reload**: reproduced by hand against `examples/click_counter`,
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
  reload discards and rebuilds the whole session, tree, and every node's
  id from scratch, so the window's underlying elements are always new to
  `gpui` on that path. `--experimental-hmr`'s in-place `rerender` is the
  first path in this project that mutates a style property on a node
  `gpui` has already seen, in the same long-lived window, with the same
  element id, and nothing else about that node changing. That points at
  `gpui`'s own element/paint reuse for a stable element id not
  accounting for a style-only change with no other difference. Not
  reproduced outside this pattern — a full reload's own repaint after a
  style edit already works, and always has.

- **`inca dev --experimental-hmr` never recovers from a source file that
  was already broken when the session started**: a live edit that
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
  general gap the next entry covers.

- **The HMR bootstrap's rejection handler doesn't distinguish who's
  responsible for reporting a failure**: a Vite/bundler-caused failure
  is Node's own event, and Node already reports it (`hmr.mts`'s
  `onError`). A genuine error in the running app's own code, unrelated
  to Vite, has no other reporter, and belongs to the app to report. The
  HMR bootstrap script (`writeHmrEntry` in `hmr.mts`) doesn't make this
  distinction: every rejected `import()` goes to `console.error(err)`
  inside the running app, regardless of which side actually caused it.
  A Vite-caused failure at startup therefore prints twice — once from
  Node, once raw and unstyled from the app (see the entry above). Fix:
  have the bootstrap recognize a Vite-shaped rejection and skip its own
  report for that one case, while still reporting anything else itself.

- **`[inca] reload`/`[inca] update`'s timing isn't the useful number**:
  both only measure how long the bundler took to prepare an update, not
  how long until the app actually applies it and the screen reflects
  it. Getting the real number needs the host to confirm back to Node
  once an update is actually applied, which no dev-protocol message
  does today.

- **A stale `node_modules/.inca/hmr/inca.json` outlives the config that
  wrote it**: `hmr()` (`packages/cli/src/adapter/vite/hmr.mts`) only writes
  the file when `serializeConfig(runtimeConfig)` returns something; if a
  later session's config no longer produces any content, the old file is
  never removed, so a stale `window`/etc. section keeps being read.

- **A `full-reload` HMR payload skips the ready/pendingReload guard, and
  reports nothing**: `dev.mts`'s `reload: () => void reloadHost()` (passed
  to `bundler.hmr`) calls straight through to `reloadHost()` regardless of
  whether the host has finished its first load — the non-HMR `onBuild`
  path guards this with `pendingReload`, but a `full-reload` payload has no
  equivalent. It also logs nothing on success, unlike the "reload ... (Nms)"
  line `onBuild` prints, so a full reload under `--experimental-hmr` is
  invisible to the user.

- **A script edit's new value doesn't reach a real click, once it
  reverts to a value already used earlier in the same session**:
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
  reuse already suspected in the entry above, this time for an event
  listener closure rather than a style property, on Vue's own `reload`
  path rather than `rerender`. Not confirmed at the native layer the way
  the style-only entry above was.

- **Commenting out a `:style` property doesn't revert it, and this is true
  of every property, not just one**: root cause is now known, and it's
  unrelated to the element/paint reuse issue above. `patchProp.mts`'s
  `patchStyle` (`packages/core/src/vue/patchProp.mts`) never reads its own
  `_prevValue` argument, and only ever calls `core.setStyle` for a
  string/number entry in the next value — a key that disappears (or turns
  `null`/`undefined`) is simply never touched, and there's no native
  `removeStyle`/`removeAttribute` to call even if it noticed. Fixing it
  spans three layers: `crates/inca-gpui` (a `remove_style`/`remove_attribute`
  on the tree), `crates/inca-bridge` (the matching bindings), and
  `packages/core` (`native.mts`/`tree.mts`/`rendererCore.mts` plus
  `patchStyle` actually diffing prev vs next). Deferred past v0.0.6.

- **Dev relay hardening (`__inca_dev__`)**: three related gaps in
  `crates/inca-bridge/src/dev.rs`. `install_dev`'s `send` accepts any method
  name, including the host's own reserved ones (`ready`, `appError`), so a
  running app can spoof a host notification. `call_dev_receive` discards
  whatever `.ok()?` swallows when reading `__inca_dev__`/`receive` off
  globals, rather than capturing it as an `EngineError`. `receive` is
  invoked via a bare `Function::call`, leaving `this` unbound instead of
  bound to `__inca_dev__` — `specs/PROTOCOL.md`'s own
  `__inca_dev__.receive?.(...)` implies a method call. Also,
  `PROTOCOL_VERSION` (`crates/inca-host/src/protocol.rs`) is still `0`
  despite this new relay surface landing; bump it or write down that it's
  a backstop that doesn't move on every feature addition.

- **The stdout writer channel is unbounded**: `StdoutWriter::spawn`
  (`crates/inca-host/src/dev.rs`) backs its writer thread with
  `async_channel::unbounded`, so a client that stops reading stdout never
  blocks the host — it grows the host's memory without limit instead.
  `specs/PROTOCOL.md` used to claim the opposite ("a client that stops
  reading eventually stops the host"); that wording is now fixed to match.
  Fixing the behavior itself means a bounded channel with real backpressure.

- **Queued stdout lines can be lost on exit**: `StdoutWriter`'s writer
  thread (`crates/inca-host/src/dev.rs`) is spawned and never joined —
  `run_bundle` (`crates/inca-host/src/app.rs`) doesn't wait for it before
  the process exits. A line still in the channel (a startup-failure
  report, say) can be dropped if the process exits first. Fix: keep the
  `JoinHandle`, drop the sender, and join the thread before `run_bundle`
  returns.

- **Auto-resize gaps beyond the v0.0.6 reload-latch fix**: `start()`
  (`crates/inca-host/src/app.rs`) never calls
  `maybe_auto_resize_to_content` after its own initial job drain — only
  `reload`/an unrecognized-notification relay do, so a first launch whose
  content mounts asynchronously (the HMR bootstrap, notably) only catches
  up once some later dev-protocol relay happens to fire. Separately,
  `maybe_auto_resize_to_content`'s `width_ready`/`height_ready` treat a
  configured dimension as ready whenever it's `Some`, without running it
  through `usable()` first, so an unusable configured value (`width: 0`)
  still latches the resize. And a root that only ever declares one
  dimension (by design, not a timing accident) never auto-resizes at all,
  since both dimensions have to become ready together.

- **`-32601` still doesn't return the method name**: `handle_unrecognized`
  (`crates/inca-host/src/dev.rs`) has the unrecognized `method` in hand but
  answers a fixed `"unknown method"` string regardless. A client can't tell
  from its own log which method got rejected.

- **A camelCase (or otherwise miscased) style key is silently ignored**:
  `style_spec_from_props` (`crates/inca-gpui/src/element.rs`)'s match falls
  through to `_ => {}` for any key it doesn't recognize, including
  `flexDirection` where `flex_direction` was meant — no error, no warning,
  the property just never applies. Someone coming from web-standard CSS/
  Vue conventions hits this as a silent no-op with nothing to point at the
  typo. At minimum this should warn in dev mode.

- **`resolveEntry` only recognizes `src/main.mts`, not `.ts`/`.js`**:
  `packages/cli/src/entry.mts` checks the literal `src/main.mts` path and
  falls back to synthesizing an entry from `src/App.vue`; an `.mts`-less
  `src/main.ts`/`src/main.js` is invisible to it. `docs/reference/elements.md`
  already tells readers this is "intended for later, not yet implemented."
  Track the actual work here: accept `.ts`/`.js` too, or write down why
  `.mts` alone is required.

- **`ERR_INCA_HOST_BIN_NOT_FOUND` is `inca package`-only; `inca dev` hits
  the same failure with no code at all**: `package.mts` resolves
  `resolveHostBin()` and then explicitly `existsSync`-checks the result,
  raising `ERR_INCA_HOST_BIN_NOT_FOUND` if it's missing. `dev.mts`'s
  `HostClient.start` (`packages/cli/src/dev-client/hostClient.mts`) calls
  `spawn()` on the same resolved path with no existence check and no
  `"error"` listener on the child process — an `INCA_HOST_BIN` pointing at
  a nonexistent binary surfaces as an unhandled `ENOENT` `"error"` event,
  not a reported `IncaError`. Either give `inca dev` the same code for the
  same failure, or write down why the two commands' error handling for an
  unresolvable host binary is meant to differ.
