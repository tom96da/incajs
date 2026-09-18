<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Implementation Plan

Granular, checkbox-tracked task breakdown that complements
[ROADMAP.md](./ROADMAP.md)'s phase-level design intent with per-task
tracking.

**Finished work is append-only.** A section whose boxes are ticked is never
rewritten or deleted, and new scope discovered mid-implementation is
appended as a new item near the relevant task rather than folded into a
done one's wording. This lets progress be read straight off the checkboxes,
with no need to keep prose in sync with reality.

**Unstarted work is a draft.** A unit nobody has begun carries no history to
protect, so it can be reworded, resplit, renumbered, or dropped whenever a
better design turns up — that is the point of writing it down early. Rework
a partly-ticked unit only by agreement; a wholly unticked one needs none.

## Phase 1: Rust host & FFI bridge core (`gpjs-ui`)

See [ROADMAP.md#phase-1](./ROADMAP.md#phase-1-rust-host--ffi-bridge-core-inca-gpui)
and [FFI.md](./FFI.md) for the design this implements.

### Prerequisites

- [x] Add native build dependencies for `gpui` to the devcontainer

### Rust workspace

- [x] Root `Cargo.toml` workspace scaffold (`gpui` as a git dependency)
- [x] `crates/gpjs-ui/Cargo.toml`

### Phase 2 landing spot (scaffolding only, no renderer logic yet)

- [x] `pnpm-workspace.yaml` + root `package.json`
- [x] `packages/vue` placeholder package (npm name `@gpjs-ui/vue`)
- [x] `packages/gpjs-ui` placeholder package (framework-agnostic; deferred — scaffold when Phase 2 implementation begins)
- [x] `examples/hello-vue` placeholder package (deferred — needs `@gpjs-ui/vue` to have real content first) — landed differently than this line originally named: two separate example apps, `examples/hello_world` and `examples/click_counter` (Phase 2 Unit iv)

### Unit i — VirtualNode arena

- [x] Implement `crates/gpjs-ui/src/tree.rs`
- [x] Unit tests: id uniqueness, append order, detach-keeps-node-alive,
      remove-of-absent-child is a no-op, set-attribute overwrites,
      unknown-id lookup returns `None` rather than panicking

### Unit ii — QuickJS runtime bootstrap

- [x] Implement `crates/gpjs-ui/src/js/engine.rs`
- [x] Tests: `ctx.eval` smoke test, syntax-error propagation,
      two engines don't share globals

### Unit iii — `__gpjsui_native__` bindings

- [x] Implement `crates/gpjs-ui/src/js/bindings.rs`
- [x] Tests driven through `ctx.eval` (happy path, type round-trips,
      bad id/type raises a catchable JS exception, not a Rust panic)

### Unit iv — minimal gpui window

- [x] `crates/gpjs-ui/examples/gpui/hello_world.rs`
- [ ] `#[gpui::test]` headless smoke test — tried, then removed: its
      `TestPlatform` doesn't exercise real rendering or fonts, so it can't
      catch the class of bug this unit cares about. `cargo check -p
      gpjs-ui --all-targets` compile-checks the example instead.
- [ ] Manual: run the example and look at the window — confirmed on macOS
      only so far; see [MANUAL_GUI_CHECK.md](./MANUAL_GUI_CHECK.md).

### Unit v — VirtualNode → AnyElement conversion

- [x] `crates/gpjs-ui/src/render/element.rs` (pure spec layer + thin gpui layer)
- [x] Tests for both layers
- [x] Test: for each of gpui's own examples (starting with hello_world),
      compare computed layout (not pixels) between the upstream version
      and the same layout built via gpjs-ui's `VirtualTree` through JS —
      should work headlessly, since layout computation alone doesn't need
      real rendering/fonts (unlike Unit iv's removed test) — landed as
      `crates/gpjs-ui/tests/layout_parity.rs`, reproducing hello_world's
      shape (not a literal copy: that's a binary example, not importable);
      only hello_world's shape is covered so far, not gpui's other examples
- [x] Manual: a new example rendering a real `VirtualTree` through gpjs-ui —
      `crates/gpjs-ui/examples/hello_world.rs`; confirmed on macOS by
      comparing side-by-side against `examples/gpui/hello_world.rs`. Layout,
      colors, and text all matched; the six squares' dashed borders didn't —
      GPUI's `border_style` (solid vs. dashed) isn't in the v1 style
      vocabulary yet, so it always renders solid
- [ ] The v1 style vocabulary (`specs/FFI.md`) is deliberately incomplete:
      percentage lengths, min/max size, margin/padding,
      flex-grow/shrink/basis, per-side border/corner values, box-shadow, and
      align/justify variants beyond start/end/center/stretch are not yet
      implemented — add as real usage needs them

### Unit vi — event-driven JS invocation (zero-overhead render loop)

- [x] `crates/gpjs-ui/src/render/bridge.rs` — `EventDispatcher`, dispatching
      `"click"` only so far (see specs/FFI.md's "Event dispatch" section);
      other event names aren't wired to real GPUI input yet
- [x] Give every container a real GPUI `ElementId` (e.g.
      `ElementId::Integer(node_id as u64)`, reusing our existing stable
      `NodeId`) before wiring up any interactivity. `build_element`
      currently assigns none — harmless today (Unit v has no interactive
      state), but a container with no id loses GPUI's
      `InteractiveElementState` (hover/active/focus/pointer-capture) across
      re-renders, since GPUI keys that state off the element's
      `GlobalElementId`. (Cross-checked against a competing GPUI-based
      framework that hit exactly this bug.) Confirmed while implementing:
      `on_click` itself doesn't exist without `.id(...)` first — this
      wasn't just about state persistence, `on_click` is only reachable via
      `StatefulInteractiveElement`.
- [x] Test: one simulated input event triggers exactly one JS call — landed
      as `crates/gpjs-ui/tests/event_dispatch.rs`'s
      `click_dispatches_to_js_exactly_once`. Dropped the "and one
      `cx.notify()`" half of this item: confirmed empirically that
      `window.refresh()` (not `cx.notify()`/`Context<V>` — no entity
      plumbing needed through `render_tree`/`build_element` at all) is what
      `dispatch()` calls, but also confirmed a click on *any* interactive
      element already makes GPUI redraw for its own hover/active
      bookkeeping regardless — so a redraw-count assertion can't actually
      isolate this call's effect, and isn't included in the test.
- [x] Test: repeated renders with no new events touch the JS engine zero
      times — `event_dispatch.rs`'s
      `repeated_builds_with_no_event_never_touch_the_js_engine` (a plain
      `#[test]`: building an `AnyElement` needs no `gpui` App/Window at
      all, confirmed while implementing)
- [x] Manual: clickable element mutates state via JS and re-renders — landed
      as a new example, `examples/click_counter.rs` (not folded into
      `examples/hello_world.rs`: that one's whole point is being a faithful,
      non-interactive recreation of gpui's own static example, so bolting
      interactivity onto it would undermine what its name/doc comment
      promise); confirmed on macOS — clicking the box counts up

### Docs

- [x] Update `specs/STRUCTURE.md` and `AGENTS.md`'s Status section to match
      what actually landed

## Phase 2: JS core bridge (`gpjs-ui`) & Vue 3 custom renderer (`@gpjs-ui/vue`)

See [ROADMAP.md#phase-2](./ROADMAP.md#phase-2-js-core-bridge-incajs--vue-3-custom-renderer-incajsvue)
and [FFI.md](./FFI.md) for the design this implements.

Scope grew beyond ROADMAP.md's four bullet points once planning dug into
the actual `@vue/runtime-core` `createRenderer` API surface: the native
bridge has two real gaps that block a working custom renderer, not just
missing polish — see Prerequisites.

### Prerequisites — native bridge gap fixes (Rust, `crates/gpjs-ui`)

`setAttribute` is currently the only JS-bound setter — there's no
JS-reachable way to touch `style_props`, even though rendering
(`element.rs`) reads style exclusively from there. And `appendChild` only
appends at the end (`Vec::push`) — `RendererOptions.insert(el, parent,
anchor)` needs to insert before a specific sibling for correct Vue list
(`v-for`) diffing.

- [x] Add `setStyle(nodeId, key, value)` to `__gpjsui_native__`
      (`src/js/bindings.rs`), mirroring `setAttribute`'s validation/
      error-mapping pattern, calling `VirtualTree::set_style`
- [x] Tests mirroring `setAttribute`'s existing ones (happy path, unknown
      node id, non-primitive value)
- [x] Add `VirtualTree::insert_before(parent_id, child_id, anchor_id:
      Option<NodeId>)` in `src/tree.rs` (`None` anchor = append at end,
      so `append_child` becomes a thin wrapper over it) — an `anchor_id`
      naming a real node that isn't currently a child falls back to
      appending at the end, only a wholly unknown id errors
- [x] Bind it as `insertBefore(parentId, childId, anchorId | null)` in
      `bindings.rs`, same error-mapping pattern as `appendChild`
- [x] Tests: insert at start/middle/end, unknown parent/child/anchor id
- [x] Update `specs/FFI.md`'s binding function table and prose to add
      `setStyle`/`insertBefore`, and correct the note that folds style
      into `setAttribute`

### Unit i — TS tooling scaffolding

- [x] Root `tsconfig.base.json`, extended by each package's own
      `tsconfig.json` (`.mts` sources, `module: "preserve"` — the
      Vite-recommended setting for library builds, whose implied
      `moduleResolution: "bundler"` still requires an `.mts` file's own
      relative imports to name a real extension — neither extensionless
      nor a `.js`/`.mjs` specifier resolves to a sibling `.mts` file
      under this mode, only the literal `.mts` extension does (see Unit
      ii for the `allowImportingTsExtensions` flag that permits it)
- [x] Vite library-mode build for both packages (`vite.config.mts`, ESM
      output, `types`/`exports` fields in each `package.json`) — verified
      end-to-end: `pnpm -r typecheck`/`build`/`test` all pass against a
      placeholder `src/index.mts`
- [x] TypeScript 7's native compiler (`typescript@^7.0.2`, no
      programmatic API until 7.1) is used for the fast `tsc --noEmit`
      typecheck; `typescript` itself is npm-aliased to
      `@typescript/typescript6@^6.0.2` (its `tsc6` binary + full API) so
      API-consuming tooling (`unplugin-dts`) keeps working — the real
      native package lives under the `@typescript/native` alias instead
- [x] Declaration output via `unplugin-dts/vite` (not `vite-plugin-dts`,
      which is now just a thin, deprecated wrapper around it — see
      https://github.com/qmhc/unplugin-dts/blob/main/docs/en/usage.md)

### Unit ii — `gpjs-ui` core package (`packages/gpjs-ui`)

- [x] Typed wrapper functions over `globalThis.__gpjsui_native__`:
      `createNode`, `appendChild`, `insertBefore` (new), `removeChild`,
      `setAttribute`, `setStyle` (new), `addEventListener`
- [x] Callback registry owning the `globalThis.__gpjsui_callbacks__[id]`
      convention internally (id allocation/cleanup) — `@gpjs-ui/vue`
      should never touch that raw contract itself — re-registering the
      same `(nodeId, event)` frees its previous callback id; a stale id
      left behind in Rust's `EventListeners` is harmless once its
      JS-side entry is gone (dispatch just finds nothing, per
      `specs/FFI.md`'s event dispatch section). Not yet handled: a
      removed node's own listeners are never freed (`NodeId`s are never
      reused — `tree.rs`'s `create_node` — and there's no native
      "node removed" hook to react to), so this only bounds growth for
      handlers that get replaced in place, not ones whose node goes
      away — revisit once Unit iii's `remove(el)` needs it
- [x] TS types for the `specs/FFI.md`-documented style-prop and tag
      vocabulary (compile-time safety on top of the native bridge's
      stringly-typed calls) — `setStyle` is overloaded so known
      `StyleProps` keys get a typed value while unrecognized keys still
      fall back to the raw `AttributeValue` signature
- [x] Vitest unit tests: each wrapper call forwards correctly to a mocked
      `__gpjsui_native__`; callback registry id lifecycle — landed as
      `packages/gpjs-ui/src/index.test.mts`
- [x] Root `tsconfig.base.json` needed `allowImportingTsExtensions` once
      a real intra-package `.mts` import showed up (the test file
      importing `./index.mts`) — confirmed by elimination: with the flag
      off, an extensionless specifier and a `.js`/`.mjs` one both fail
      with `TS2307` (`module: "preserve"`'s implied `moduleResolution:
      "bundler"` doesn't extend `NodeNext`'s `.js`→`.ts`/`.mjs`→`.mts`
      mapping to this mode), so the literal `.mts` extension plus this
      flag is the only working spelling; the flag's own precondition
      (`noEmit`/`emitDeclarationOnly`) already holds via `noEmit: true`
- [x] `unplugin-dts`'s `include: ["src"]` also picked up `*.test.mts`
      files, emitting a stray `dist/index.test.d.mts` — fixed by adding
      `exclude: ["src/**/*.test.mts"]` to both packages'
      `vite.config.mts`

### Unit iii — `@gpjs-ui/vue` custom renderer (`packages/vue`)

`nodeOps` implements `@vue/runtime-core`'s `RendererOptions<HostNode,
HostElement>` — its 9 required methods (`createElement`, `createText`,
`createComment`, `insert`, `remove`, `setText`, `setElementText`,
`parentNode`, `nextSibling`); the 4 optional ones (`querySelector`,
`setScopeId`, `cloneNode`, `insertStaticContent`) are out of scope, not
needed without SSR/hydration. Built on `packages/gpjs-ui`, never on
`__gpjsui_native__` directly.

- [x] `createElement`/`insert`/`remove` — the core node lifecycle, built
      on `packages/gpjs-ui`'s `createNode`/`insertBefore`/`removeChild` —
      landed as `packages/vue/src/nodeOps.mts`. `HostNode`/`HostElement`
      aren't the bare `NodeId`: each is a JS object (`GpjsuiElement`/
      `GpjsuiText`) pairing the native id with the parent/children
      bookkeeping the next item describes, matching the shape
      `@vue/runtime-test`'s reference renderer uses for its own host nodes
- [x] Add `disposeNode(nodeId)` to `packages/gpjs-ui` (closes the gap
      Unit ii's callback registry left open — see its notes above): purges
      every callback id registered for `nodeId` from
      `registeredCallbackIds`/`__gpjsui_callbacks__`, across all event
      names, not just the one currently patched. `nodeOps.remove(el)`
      calls it alongside `removeChild` — this is the first point in the
      stack that actually knows a node is gone for good
- [x] `createText`/`setText`/`setElementText` map to a `tag: "text"` node + `setAttribute(id, "value", text)`, matching `hello_world.rs`'s
      existing text-leaf convention
- [x] `createComment` maps to a hidden node (`createNode("div")` +
      `setStyle(id, "display", "none")`) — `display: none` is already in
      the style vocab, so this needs no render/`element.rs` change
- [x] JS-side parent/sibling bookkeeping map maintained as `nodeOps`
      processes `insert`/`remove`, answering `parentNode`/`nextSibling`
      and giving `remove(el)` the parent id `removeChild` needs (the
      native tree has no parent pointers, and `RendererOptions.remove`
      doesn't pass one) — `insert` also detaches a child from its old
      parent first (in both the native tree and this bookkeeping) before
      attaching it to the new one, so moving a node for a keyed-list
      reorder doesn't dispose it
- [x] `patchProp`: `key === "style"` (an object, per Vue's
      `:style="{...}"` binding) → one `setStyle` call per entry, unknown
      keys silently ignored (matching `specs/FFI.md`'s existing
      "malformed/unrecognized → ignored" policy) — a value shape
      `setStyle` can't take (non-primitive) is skipped the same way,
      since unlike a style prop's own render-path fallback, `setStyle` is
      a JS call boundary that would otherwise raise
- [x] `patchProp`: `isOn`-prefixed keys (`onClick`, ...) → register via
      the callback registry + `addEventListener` — only `"click"` is
      wired to real input today (`specs/FFI.md` v1), other `on*` props are
      inert for now
- [x] `patchProp`: everything else → `setAttribute`, skipping non-primitive
      values the same way `style` does (removing a prop entirely — a
      `null`/`undefined` next value — is also a no-op: there's no native
      "unset" call to route it to, a deferred v1 gap like the others above)
- [x] `createGpjsuiApp(App)` convenience wrapper around
      `createRenderer(nodeOps).createApp` — mounts against a
      `HostElement`-typed root handle (see Unit iv for how the Rust host
      supplies the root node id)
- [x] Vitest tests asserting the create/insert/patchProp/remove call
      sequence against a mocked `gpjs-ui` core (mirroring the pattern
      `@vue/runtime-test`'s reference renderer uses for its own tests) —
      `packages/vue/src/nodeOps.test.mts`/`patchProp.test.mts`. Also added
      (beyond this item's original scope): `packages/vue/tests/
      renderer.test.mts`, an integration test driving `createGpjsuiApp`
      end-to-end through a real (unmocked) `gpjs-ui` core against a small
      in-memory fake standing in for `globalThis.__gpjsui_native__`,
      confirming a mounted component's styles/attributes/text and a
      keyed-list reorder (moves, not recreation) — the scenario that
      originally motivated this phase's `insertBefore` prerequisite

### Unit iv — Vue example apps + `gpjs-ui-example-runner` (manual GUI check)

Landed differently than originally planned on this line (one shared
`examples/hello-vue` package + a Cargo example living inside `crates/gpjs-ui`)
— see the design notes below for what changed and why, arrived at over
several rounds of review.

Each `.vue` SFC compiles via a minimal one-shot `@vue/compiler-sfc` script —
no Vite yet (that's Phase 3's job); only the thin "invoke once" wrapper
becomes obsolete when Phase 3 swaps in Vite's dev pipeline, the
`compileScript`/bundling calls themselves carry forward unchanged.

- [x] Two independent, pure-Vue/TS example apps, each its own pnpm
      workspace package with no Rust inside — `examples/hello_world`
      (recreating `hello_world.rs`'s static tree: bordered box, text
      label, a `v-for` row of six colored squares) and
      `examples/click_counter` (recreating `click_counter.rs`'s clickable,
      counting box, via `ref`/`computed`)
- [x] Each app's `scripts/build.mjs`: one-shot `@vue/compiler-sfc`
      `compileScript({ id, inlineTemplate: true, templateOptions:
      { compilerOptions: { runtimeModuleName: "@vue/runtime-core" } } })`
      (retargeted off the default `"vue"` specifier — this repo has no
      `vue` meta-package dependency anywhere), then a programmatic `vite
      build()` (`define` stubs `process.env.NODE_ENV`, since
      `@vue/runtime-core`'s dev-only warning branches check it directly
      and QuickJS has no Node globals) bundling the compiled SFC together
      with `@gpjs-ui/vue` and `@vue/runtime-core` into one self-contained
      `dist/bundle.js` — no unresolved imports, no Vite dev server
- [x] `Engine::eval_module` (`src/js/engine.rs`): `rquickjs::Module::declare`
      + `.eval()` + `Promise::finish()` inside the existing `Context::with`
      closure — no `ModuleLoader`/resolver needed, since the bundle this
      evaluates is fully self-contained. Returns `EngineResult<()>`, not a
      module's exports: a module's own completion value is always
      `undefined` per spec, so unlike `eval` there's nothing meaningful to
      convert to a caller-chosen type; state comes back through
      `globalThis`, the same convention `click_counter.rs` already uses
      for plain scripts
- [x] `crates/gpjs-ui/tests/js_core_integration.rs`: install real
      `__gpjsui_native__` bindings via `bindings::install`, then — through
      `Engine::with` directly rather than `eval_module`, since this needs
      the module's export table — `Module::declare`/`.eval()`/
      `Promise::finish()` the bundled `packages/gpjs-ui` output and call
      its exports back via `Module::get`, asserting against the resulting
      `VirtualTree` state. A real bundler renames a module's internal
      top-level bindings (confirmed empirically), so exports are only
      reachable this way, not by assuming a hand-written-JS-style
      same-scope call works against real compiled output
- [x] New crate `crates/gpjs-ui-example-runner` (**not** `crates/gpjs-ui/examples/gpui/hello_vue.rs`
      as originally planned): a single generic loader, `gpjs-ui-example-runner
      <path-to-bundle.js>`, used to run *either* example app. Creates one
      root `VirtualNode`, installs bindings, reads the bundle, substitutes
      a `__GPJSUI_ROOT_ID__` placeholder token with the real root id via
      `str::replace` (not `format!` — the bundle is a large file full of
      literal `{`/`}`, unlike `click_counter.rs`'s short inline literal),
      `eval_module`s it, then opens a GPUI window sized off the mounted
      root's own `width`/`height` style (falls back to 800×600 if unset)
      and always renders via `render_tree_with_events` + `EventDispatcher`
      (never plain `render_tree` — one binary can't know ahead of time
      whether a given bundle registered any click handlers)
- [x] `EventDispatcher::dispatch` (`src/render/bridge.rs`) bugfix, found
      while manually checking `click_counter`'s real Vue port: a callback
      that only *schedules* its effect via a microtask (as
      `@vue/runtime-core`'s reactivity scheduler does, batched via
      `Promise.resolve().then(...)`) hadn't actually mutated the tree by
      the time `dispatch` returned — nothing ever drained the pending job
      that would run it. Fixed by draining pending jobs right after
      calling the registered callbacks; regression test added to
      `tests/event_dispatch.rs` using a deferred callback
- [x] `packages/vue/package.json`: `@vue/runtime-core` moved from
      `dependencies` to `peerDependencies` (+ `devDependencies`, for this
      package's own build/test) — otherwise a consuming app could resolve
      its own separate copy of `@vue/runtime-core`, producing a duplicate
      Vue instance with reactivity split across the two copies
- [x] Manual: run the example and look at the window — confirmed
      end-to-end on both macOS (native) and from the devcontainer via
      XQuartz forwarding, both examples, per
      `specs/MANUAL_GUI_CHECK.md`

### Docs

- [x] Update `AGENTS.md`'s Status section once Phase 2 lands

## Phase 3.1: `gpjsui dev` (full reload)

See [ROADMAP.md#phase-31](./ROADMAP.md#phase-31-inca-dev-full-reload) for
the design this implements, and its Phase 3 preamble for why orchestration
lives on the JS side. Phases 3.2–3.4 get their own sections when they start.

### Prerequisites — CI workflow

Lands before any 3.1 code: this is the first phase to touch two crates and
three packages at once, and it's the last chance to add CI before there's a
release to protect.

- [x] `.github/workflows/ci.yml` running exactly
      [TESTING.md](./TESTING.md)'s required checks — that doc stays the
      single source of truth for what must pass, the workflow just runs it
- [x] Rust job matrix over `ubuntu-latest` + `macos-latest` (macOS is the
      primary development target and `gpjs-ui`'s `gpui_platform` features
      differ per platform — `font-kit` vs. `wayland`/`x11` — so one runner
      can't compile-check both paths)
- [x] Linux runner installs `gpui`'s native build dependencies; keep the
      list derived from `.devcontainer/Dockerfile`'s, not independently
      invented
- [x] TypeScript job on `ubuntu-latest` only (`pnpm -r test` already covers
      lint/format/typecheck via each package's `pretest`)
- [x] `cargo test -p gpjs-ui` needs `pnpm --filter gpjs-ui build` to run
      first (`tests/js_core_integration.rs` reads `dist/index.js` off disk)
- [x] No submodule checkout: `third_party/` is reference-only, and `gpui`
      comes from a git dependency, so the default shallow checkout is enough
- [x] Cache the cargo build and the pnpm store
- [x] Update `specs/STRUCTURE.md`'s tree with `.github/workflows/`
- [x] `examples/*` define no `pretest`, so `pnpm -r test` alone doesn't lint
      or type-check them — a separate `lint` job runs the root `pnpm lint`,
      `pnpm format`, and `pnpm typecheck` as three named steps, which do
      cover `examples/`
- [x] Vitest runs across Node 20/22/24/26, each test job installing and
      testing on its own version directly. Node 20 has no native `.mts`
      execution, so its job installs `tsx` and runs the test fixtures
      through it. A separate `build` job (Node 26) builds the workspace
      once; `lint` and the test jobs depend on it via `needs:`. The root
      `package.json`'s `engines.node: ">=22"` is the oldest Node still
      under LTS (20 reached end-of-life 2026-04-30; it stays in the test
      matrix but isn't required) and only constrains this (private)
      workspace; no published package declares an `engines` floor yet —
      settle one when Phase 3.3 actually publishes
- [x] `permissions: {}` and `persist-credentials: false`, from Vite's
      workflow. Actions stay **tag**-pinned rather than SHA-pinned, though:
      this workflow uses no secrets and the token is left read-only, so a
      hijacked tag (as in the March 2025 `tj-actions/changed-files` incident)
      has little to reach here. Phase 3.3's CD workflow will hold an npm
      publish token — pin by SHA there
- [x] A `CI Passed` job aggregates the others, so branch protection has one
      required check whose name survives changes to either matrix
- [x] A `changed` job gates the rest by path, so a docs-only change skips
      both toolchains. Deliberately not `on: paths:` — that stops the
      workflow from running at all, leaving a required check unreported and
      the pull request unmergeable, whereas a skipped job reports success
- [x] Confirm the first run is green on GitHub — every check passes locally,
      but the workflow itself has never run

### Unit i — native root handle

Replaces Phase 2 Unit iv's `__GPJSUI_ROOT_ID__` source substitution, which
can't survive a CLI that doesn't know about the token.

- [x] `rootNodeId()` binding in `crates/gpjs-ui/src/js/bindings.rs`,
      following `setStyle`'s validation/error-mapping pattern
- [x] Tests mirroring the existing binding tests
- [x] `specs/FFI.md`: add it to the binding table; re-apply the FFI safety
      checklist below
- [x] `packages/gpjs-ui`: typed wrapper + unit test
- [x] `packages/vue`: `createGpjsuiApp(App).mount()` callable with no
      argument, defaulting to the native root — `src/createApp.mts` is
      currently a type-fixed re-export of `renderer.createApp`, so this
      becomes a real wrapper function

### Unit ii — `crates/gpjs-ui-host`

- [x] Rename `crates/gpjs-ui-example-runner` to `gpjs-ui-host`, keeping the
      existing `<path-to-bundle.js>` one-shot behaviour intact
- [x] `specs/PROTOCOL.md`: the message surface between the host and the Node
      process that spawns it
- [x] Dev mode: read newline-delimited JSON on stdin, write protocol
      messages on stdout, log to stderr
- [x] Handle `shutdown` by closing the window and exiting 0, so an app's
      cleanup hook has somewhere to attach without a later protocol change
- [x] Route stdin off the GPUI main thread — `gpui`'s `AsyncApp` isn't
      `Send` (it holds a `Weak<AppCell>`), so a reader thread hands messages
      to a foreground task via a channel rather than touching the app
- [x] Reload: rebuild the `Engine` (the reliable way to discard all QuickJS
      state), reset the `Host`, recreate the root node, re-evaluate the
      bundle, then refresh the window
- [x] Factor the "run JS, drain pending jobs, refresh" sequence out of
      `EventDispatcher::dispatch` (`src/render/bridge.rs`) so reload uses it
      too — today the drain is inline and skipped when no listener fires
- [x] Keep `crates/gpjs-ui` free of process/IPC concerns: new dependencies
      belong to the host crate only
- [x] Tests: a reload leaves no stale tree nodes, listeners, or JS callbacks
- [x] Tests: protocol line parsing
- [x] `EngineError` carries the thrown value's message and stack, read
      inside the call that failed so a caller cannot get the order wrong. A
      thrown value that isn't an `Error` has no stack to report
- [x] `crates/gpjs-ui` reports an uncaught listener exception through an
      injected reporter rather than swallowing it, defaulting to stderr —
      taking the reporter as an argument is what keeps it free of the
      protocol
- [x] Point that reporter at an `appError` notification in dev mode
- [x] Write protocol lines through an injected writer rather than
      `io::stdout()` directly. A slow reader then cannot block the thread
      that runs the app, and a test can read back what was sent
- [ ] Show a failed reload or build in the window, drawn by a dev-only
      engine of the host's own rather than by the app's. It outlives the
      reload that replaces the app's engine, leaves the app's tree alone,
      and can still draw when the first bundle never loaded at all
- [ ] That engine renders into a second root the host stacks over the
      app's — the same primitive an app's own modal needs. `position`,
      `z_index` and `overflow` are absent from the style vocabulary, so it
      waits on Phase 4
- [x] Tests: a listener that throws is reported exactly once — to the dev
      channel or to stderr, never both — and the window keeps rendering.
      Needs the injected writer above — landed as
      `a_throwing_listener_is_reported_exactly_once_to_the_writer` in
      `crates/gpjs-ui-host/src/main.rs`, dispatching a real click against a
      running `HostedApp` window and reading back the captured line
- [x] Speak JSON-RPC 2.0: its `id` keeps a response matched to the request
      that caused it, and its error codes are already settled

### Unit iii — core corrections

`crates/gpjs-ui` gaps a dev loop running a real app continuously makes
unavoidable. Unit ii's error reporting depends on the `console` and reporter
work here.

- [x] A node has one parent: `insertBefore` detaches it from the previous
      one, and an attachment that would make a node its own ancestor throws —
      a cycle built from JS has no bottom for the render walk
- [x] `destroyNode` frees a whole subtree and returns the callback ids it
      released — Vue's unmount calls `remove` only on the subtree's root, so
      nothing else can reach the descendants
- [x] `removeEventListener`, and `addEventListener` no longer stacking a
      second entry for the same id — a replaced listener stayed registered
      for the life of the engine
- [x] `packages/gpjs-ui`: `destroyNode`/`removeEventListener` wrappers, in
      place of `disposeNode` clearing only the JS half
- [x] `packages/vue`: `remove` and `setElementText` destroy rather than
      detach, and the fake host in its tests matches the real one's
      registration semantics
- [x] `crates/gpjs-ui-jsenv`: the host objects installed into the QuickJS
      realm, depending on `rquickjs` alone. `console` is the first, rendering
      values the way `util.inspect` does rather than `JSON.stringify`, which
      throws on a cycle and drops functions and `undefined`
- [x] Install `console` from the host, so it exists before any bundle runs,
      and point it at stderr — stdout is the protocol channel
- [x] Only a node with a registered listener gets a GPUI element id and a
      click handler. GPUI inserts a hitbox for every element carrying a click
      listener, so wiring all of them costs a hitbox and two mouse listeners
      per node per frame
- [x] Drain the JS job queue once the window is up, and after every dispatch
      rather than only when a listener ran. Vue's post-flush queue is a
      microtask, so `onMounted` and `flush: 'post'` watchers otherwise wait
      for the first click — forever, in an app that registers none
- [x] Tests for each of the above

### Unit iv — `@gpjs-ui/host-client`

The Node end of [PROTOCOL.md](./PROTOCOL.md), and the only package that
speaks it. Depends on no bundler, so a bundler other than Vite is a new
integration registered against this channel rather than a change here.

- [x] `packages/host-client` (npm name `@gpjs-ui/host-client`), following
      the existing `packages/*` conventions (`vite.config.mts`,
      `unplugin-dts`, `pretest`)
- [x] Host binary resolution: env var → the workspace's own build →
      (later) a per-platform npm package — landed as `resolveHostBin` in
      `packages/host-client/src/index.mts`; the per-platform npm package is
      Phase 3.3 Unit i's job, not this one's
- [x] Spawn and supervise the child: launch it with `--dev <bundle>`, relay
      its stderr, and on teardown send `shutdown` before falling back to a
      kill — `HostClient.start`/`stop`
- [x] Line framing and typed messages both ways, correlating each response
      to the request it answers by JSON-RPC `id` — `HostClient.call`,
      keyed by a `Map<id, PendingCall>`
- [x] Treat a stdout line that isn't a JSON-RPC message as stray output from
      a dependency: log it and keep reading, never fail the stream —
      `HostClient`'s line handler falls back to `onStderr` for anything that
      doesn't parse as `{"jsonrpc":"2.0",...}`
- [x] Surface `appError` to the caller through its own callback
      (`onAppError`) rather than the generic notification handler that used
      to carry it — landed alongside removing that generic handler
      entirely, replaced by `onReady`/`onAppError`/`integrations`
- [x] Route a `method` this package doesn't handle to whichever integration
      registered that name, `params` untouched and unparsed — the rule that
      keeps `vite` out of its manifest; an unclaimed method name is logged
      to `onStderr` rather than silently dropped
- [x] Check `ready`'s `params.protocol` against the revision this package was
      built for (`HOST_PROTOCOL_VERSION`, mirroring the host's own
      `PROTOCOL_VERSION`), and on any difference report it through
      `onStderr` and kill the child, awaiting its exit
- [x] Vitest tests against a mock host process — one fixture per scenario
      (`appError`, an unrecognized method, a protocol mismatch), alongside
      the existing shared `mock-host.mts`/`mock-host-wedged.mts`

### Unit v — `@gpjs-ui/vite`

The Vite half of the build, and the only Node package that imports `vite` —
Phase 3.4's `@incajs/vite-runtime` is the other side of the same tool,
running inside QuickJS. A `@incajs/rspack` would be a sibling of this one,
not a rewrite.

- [x] `packages/vite` (npm name `@gpjs-ui/vite`), following
      the existing `packages/*` conventions (`vite.config.mts`,
      `unplugin-dts`, `pretest`)
- [x] Library/watch build producing one self-contained bundle, compiling
      `.vue` via `@vitejs/plugin-vue` instead of the examples' hand-rolled
      `@vue/compiler-sfc` call — it isn't in the lockfile yet, so add it.
      Keep `define`-ing `process.env.NODE_ENV` (QuickJS has no `process`) —
      landed as `watch()` in `src/watch.mts`. `@vitejs/plugin-vue` itself
      imports the `vue` meta-package unconditionally (confirmed by reading
      its published output), so `vue` is a peer dependency here despite
      this repo's own apps never depending on it — see the package's
      README for why that's still true past this one package's own
      `node_modules`
- [x] Announce each rebuild and stop there: this package never starts,
      reloads, or talks to the host
- [x] Announce a failed build too. A broken edit that never produces a
      bundle is the most common way a screen stops updating, and nothing
      downstream hears about it otherwise
- [x] Build dev with `NODE_ENV=development`, so `@vue/runtime-core`'s own
      warnings survive — they are compiled out at production, and the host
      now has a `console` for them to reach
- [x] No first-party dependency at all — the output path and, in Phase 3.4,
      the channel to answer `fetchModule` over, arrive as arguments. That is
      what makes it swappable, and it is checkable in its manifest
- [x] Vitest tests — compiling a `.vue` file, rebuilding on change, and
      reporting a syntax error without throwing

### Unit vi — `@gpjs-ui/cli`

The only package that knows the other two exist. It owns the `Bundler`
contract the adapters satisfy and injects one, so swapping bundlers is a
dependency change here rather than an edit anywhere else.

- [x] `packages/cli` (npm name `@gpjs-ui/cli`, `bin: { gpjsui }`), following
      the existing `packages/*` conventions (`vite.config.mts`,
      `unplugin-dts`). `pretest` turned out to be a root-only script, not a
      per-package one — no sibling package has its own, so this one doesn't
      either
- [x] Export the `Bundler` contract as a type (`src/bundler.mts`), kept
      independent of `@gpjs-ui/vite`'s own `WatchOptions`/`Watcher` rather
      than imported from there, even though a single adapter exists today
      — extracting a types-only package is still the move if that changes
- [x] `gpjsui dev`: bundler watch → start the host through
      `@gpjs-ui/host-client` once the first bundle lands → reload on each
      rebuild — `src/dev.mts`
- [x] App entry resolution: no entry point required. `src/App.vue` alone is
      wrapped in a synthesized `createGpjsuiApp(App).mount()`; a committed
      `src/main.mts` overrides that outright — `src/entry.mts`
- [x] Name `@gpjs-ui/vite` as the default in exactly one place — `src/dev.mts`'s
      `bundler ?? { watch }`
- [x] Terminal behaviour: print what the client relays, and turn Ctrl-C into
      a graceful teardown — `src/cli.mts`'s `SIGINT` → `AbortController`
- [x] Print an `appError` where a developer will see it, stack included and
      told apart from the app's own `console` output — the two already ride
      distinct `HostClientOptions` callbacks, so no extra bookkeeping was
      needed to tell them apart
- [x] Show a failed reload as well: the `-32000` answering the `reload` the
      CLI itself sent is what says why the screen did not change
- [x] Settle the app lifecycle surface before anything dispatches one:
      which moments an app can hook (process exit, and a reload discarding
      the session), whether a handler cancels or only observes, what it may
      await given QuickJS drains microtasks but has no timers or I/O, and
      whether `packages/gpjs-ui` wraps the root-node listener in a named
      function. Recorded in `specs/FFI.md`'s "App lifecycle surface" section
      — design only, nothing dispatches either event yet
- [x] Vitest tests — `src/entry.test.mts`, `tests/dev.test.mts`, mock host
      fixtures under `tests/fixtures/`

### Unit vii — examples migration

- [x] Drop `examples/*/scripts/build.mjs` in favour of the CLI, and remove
      `__GPJSUI_ROOT_ID__` from their entry points. Each example's SFC is
      now `src/App.vue`, so `gpjsui dev` builds and starts it with no
      config; `__GPJSUI_ROOT_ID__` was already gone (Unit i's native root
      handle superseded it before this unit started)
- [x] Update `specs/MANUAL_GUI_CHECK.md` for the new command
      (`cargo build -p gpjs-ui-host` + `pnpm --filter <name> dev` replaces
      the old build-then-`cargo run` two-liner). `specs/TESTING.md` needed
      no change — its `examples/` mentions are all about the Cargo
      examples, unrelated to these Vue ports
- [x] Manual: edit a `.vue` file and confirm the window remounts (state loss
      is expected here — that's what Phase 3.4 fixes) — confirmed with
      `hello_world` on both backends: natively on macOS, and on Linux
      (the devcontainer) via XQuartz

### Docs

- [ ] Update `AGENTS.md`'s Status section once Phase 3.1 lands

## Phase 3.2: `gpjsui build`

See [ROADMAP.md#phase-32](./ROADMAP.md#phase-32-inca-build). This is 3.1's
pipeline with the watcher removed and production settings on, so the work is
mostly about what `dev` and `build` must *share* rather than new machinery.

### Unit i — the `build` command

- [x] `gpjsui build`: one-shot build through the same `Bundler` the `dev`
      command uses, with `NODE_ENV=production`, minification on, and no host
      spawned
- [x] Factor the shared config/entry resolution so `dev` and `build` can't
      drift into producing differently-shaped bundles
- [x] Non-zero exit and a readable error on build failure — this is the
      command CI and the future release workflow call
- [x] `outDir` is never emptied (`@gpjs-ui/vite`'s `watch()` sets
      `emptyOutDir: false` — `outDir` sits outside `root` in this project's
      layout, so Vite would default to that anyway, just without the
      warning). Harmless today since the only output is one fixed-name
      `bundle.js`, but a one-shot production build is exactly where a
      leftover file from an old build shouldn't linger — decide whether
      `build` clears `outDir` itself before writing

  Decided: `resolveViteConfig` sets `emptyOutDir: mode === "production"`,
  so `build` clears `outDir` and `watch` doesn't.

### Unit ii — examples and docs

- [x] `examples/*` switch their `build` script to `gpjsui build`, and
      `scripts/build.mjs` is deleted (3.1 already stopped using it)
- [x] Update `specs/TESTING.md`'s required checks if the build command moves
- [x] Update `AGENTS.md`'s Status section

### Unit iii — module resolution, multi-file builds, and the `incajs/vue` merge

Found while moving `incajs/vue` from a thin re-export of a separate
`@incajs/vue` package into `packages/core/src/vue` — the design
[ROADMAP.md](./ROADMAP.md#phase-2-js-core-bridge-incajs--vue-3-custom-renderer-incajsvue)
always specified, but never actually landed that way. The merge produces a
shared chunk between `incajs`'s and `incajs/vue`'s built entries, which
`inca-jsenv`'s engine couldn't load: it had no `ModuleLoader`, so every
evaluated source had to be a single, self-contained module. That same
constraint is what Unit i above flagged and deferred — a dynamic `import()`
or a `.vue` file's own `<style>` block silently broke `inca build`, since
neither the CLI nor the host could handle more than one output file.

- [x] `crates/inca-jsenv`: `Engine::builder()` resolves `import`s against
      real files on disk (`DiskResolver`/`DiskLoader`), so an evaluated
      module's own imports — including a shared chunk — resolve instead of
      requiring self-contained source
- [x] `crates/inca-host`: reads an entry path rather than a single bundle
      string, and lets the loader above resolve whatever it imports
- [x] `@incajs/cli`: `Bundler.build`/`.watch` report every file a build
      wrote (`BuildOutput`), not one assumed bundle path; `inca package`
      copies the reported files rather than a single `bundle.js`; `inca dev`
      prunes stale files between rebuilds, since `outDir` is no longer
      emptied on every one
- [x] `packages/vue` deleted; its source moved into `packages/core/src/vue`,
      the `IncaCore` contract moved to `packages/core/src/rendererCore.mts`
      (derived from `index.mts`'s real exports via `Pick<typeof import(...),
      ...>` rather than hand-duplicated), and `createIncaApp` is a
      single-argument call again — the two-package split, and its
      dependency-injected `core` argument, existed only to avoid a circular
      dependency a single package no longer has
- [x] `specs/*.md` renamed off the old `gpjs-ui`/`gpjsui` codename, and its
      paths fixed where the later `inca-gpui`/`inca-bridge`/`inca-jsenv`
      split had left them stale (outside this file's own completed history)
- [x] Manual: `inca dev`/`build`/`package` still launch and render
      correctly on macOS after the loader and multi-file build changes
      above

Deliberately deferred, tracked as follow-up rather than done here:
precompiling the bundle to QuickJS bytecode (a `loader::Bundle` alongside
the disk resolver above), and a real CSS path for `.vue`'s `<style>`
blocks (Vite already emits the file; nothing in the host reads it yet).

## Phase 3.3: Application packaging and the `v0.0.1` release

See [ROADMAP.md#phase-33](./ROADMAP.md#phase-33-application-packaging).
The first release milestone: after this, the framework is publishable.

### Unit i — host binary distribution

- [x] Per-platform npm packages carrying a prebuilt `gpjs-ui-host`, selected
      by `optionalDependencies` — `npm/<os>-<arch>/`, four of them
      (`darwin`/`linux` × `arm64`/`x64`), kept out of `packages/*` since
      they carry no source or tests of their own
- [x] `@gpjs-ui/cli` resolves the host through those packages, falling back
      to the workspace build during development
- [x] Keep the host binary swappable rather than baked into the CLI —
      app-owned Rust extensions (roadmap Phase 13) depend on being able to
      substitute a locally compiled host

### Unit ii — packaging a distributable app

- [x] `gpjsui package`: pairs a production bundle with the prebuilt host and
      emits a platform-native application — a `.app` on macOS, a plain
      directory on Linux, with each other platform's target following its
      support in roadmap Phase 11
- [x] App metadata (display name, identifier, icon, version) sourced from
      the app's own `package.json` plus a `"gpjsui"` key, not hard-coded
- [x] Decided: the host reads its bundle from beside its own executable —
      `gpjs-ui-host` itself now falls back to an exe-relative search
      (`bundle.js` beside it, or `../Resources/bundle.js`) when launched
      with no arguments, covering both layouts above without either one
      needing a wrapper script
- [x] Confirmed on Linux: a packaged `examples/click_counter` finds its
      bundle and starts with no arguments, and fails with a clear message
      when the bundle is missing — this container has no display to
      confirm pixel content beyond that (see
      [MANUAL_GUI_CHECK.md](./MANUAL_GUI_CHECK.md))
- [x] Manual: launch a packaged `examples/click_counter` on macOS, outside
      any terminal, and confirm it behaves like the dev run

### Unit iii — CD workflow and the release

- [x] `.github/workflows/cd.yml`: `workflow_run`-gated on `ci.yml`,
      builds the host per platform, then publishes to npm
- [x] Publish set is npm only — `incajs`, `@incajs/cli`, and the
      per-platform host packages (`darwin-x64` excluded, no free Intel
      macOS runner). Rust crates stay `publish = false`
- [x] Workspace versioned at `0.0.1`
- [x] Each published package has its own README, generated `LICENSE`,
      and `files`/`exports` correctness — verified by packing
- [ ] Update `README.md` with real install/usage instructions
- [x] Update `AGENTS.md`'s Status section
- [x] `v0.0.1` published to npm and GitHub Releases

## Phase 3.4: HMR (`@incajs/vite-runtime`)

See [ROADMAP.md#phase-34](./ROADMAP.md#phase-34-hmr-incajsvite-runtime)
and [ARCHITECTURE.md](./ARCHITECTURE.md#hmr-delivery) for the design.
The hard part of Phase 3: it replaces 3.1's whole-bundle re-evaluation with
module-granular updates that preserve component state.

### Prerequisites — QuickJS gaps

Vite's module runner assumes a richer host environment than the engine
currently provides. Each of these is small on its own; together they're the
reason this unit comes first.

- [ ] An evaluator entry point for non-ESM code: Vite's SSR transform emits
      an async *function body* taking the six `__vite_ssr_*` parameters, so
      `Engine::eval_module`'s `Module::declare` path doesn't apply
- [ ] A `console` shim — the module runner and Vue's dev build both log
      through it, and QuickJS has none
- [ ] Job-queue pumping while a JS promise awaits a host round-trip:
      `__vite_ssr_import__` resolves only after the CLI answers, so
      something has to drive the queue between messages
- [ ] Re-apply the FFI safety checklist below to every new binding

### Unit i — `@incajs/vite-runtime`

- [ ] `packages/vite-runtime` (npm name `@incajs/vite-runtime`), declaring
      `vite` as a peer dependency — under pnpm a package only resolves what
      it declares, and this one imports `vite/module-runner` directly
- [ ] A `ModuleRunnerTransport` bridging to the host's stdio channel:
      `invoke` for `fetchModule`/`getBuiltins`, plus `connect`/`send` for
      HMR payloads (HMR requires `connect`; an invoke-only transport can't
      have it)
- [ ] A `ModuleEvaluator` running transformed code inside QuickJS, with
      `sourcemapInterceptor: false` and an `import.meta` factory that
      doesn't reach for Node APIs
- [ ] Externalized modules have no dynamic `import()` to fall back on in
      QuickJS — force everything through the transform pipeline instead of
      implementing `runExternalModule`

### Unit ii — CLI and host wiring

- [ ] `@incajs/vite` holds a real Vite dev environment in `dev`,
      answering `fetchModule` and pushing HMR payloads over the channel the
      CLI hands it, in place of 3.1's rebuild-and-reload message
- [ ] The host keeps one long-lived engine across updates — the point of
      HMR is that it *isn't* 3.1's teardown
- [ ] The window and root node survive an update; a redraw is requested
      after each applied update

### Unit iii — Vue HMR

- [ ] `@vitejs/plugin-vue`'s HMR support needs Vue's dev build
      (`__VUE_HMR_RUNTIME__` only exists there), so the dev pipeline can no
      longer hard-code `NODE_ENV=production` the way Phase 2's examples did
- [ ] Confirm `@vue/runtime-core`'s HMR rerender/reload path drives the
      custom renderer correctly
- [ ] Keep engine/window/root initialization out of the hot module graph,
      and out of module scope — a re-evaluated module must not be able to
      open a second window or orphan the renderer
- [ ] Revisit `packages/vue`'s `sideEffects: false`: an `import.meta.hot`
      block or any self-registering module scope makes the flag untrue,
      and a bundler that trusts it drops the module whole — narrow it to
      an array or drop it, depending on where the HMR hooks land

### Unit iv — tests and manual check

- [ ] A test asserting state actually survives an update, not just that no
      error was raised: a mis-wired refresh runtime fails silently, leaving
      a stale UI with no error anywhere
- [ ] Manual: edit a `.vue` file while `click_counter` is running and
      confirm the count is preserved across the update
- [ ] Update `AGENTS.md`'s Status section

## Evergreen checklists

Re-apply these on every relevant future PR — they are not phase-scoped and
never get "checked off" permanently.

### FFI safety review (any PR touching `crates/inca-bridge/src/*`)

- [ ] No `rquickjs::Value`/`Ctx<'js>`/`Persistent<T>` is stored in
      `VirtualTree`, the event-listener registry, or any other long-lived
      struct
- [ ] Every JS argument is converted to an owned plain Rust type before
      touching shared state
- [ ] Non-primitive values at typed boundaries (e.g. `setAttribute`) raise a
      catchable JS exception, not a panic or silent coercion
- [ ] Unknown/stale ids from JS are handled as `None`/a JS exception, never
      a Rust panic
