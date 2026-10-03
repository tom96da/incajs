<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Roadmap

The phases of the build-out, with the gate each one waits on.
[PLAN.md](./PLAN.md) has the per-task breakdown.

Vue 3 support is built first, end to end, in Phases 1–3. The `v0.0.1` release
lands inside Phase 3.3. Phases 4–6 add what any app needs: input,
accessibility and a runtime library. Phases 7–13 broaden the surface: styling,
developer tools, platform integration, a second frontend framework and
cross-platform support.

| Phase | Scope |
| --- | --- |
| 1 | Rust host & FFI bridge core |
| 2 | JS core bridge & Vue 3 custom renderer |
| 3 | Developer tooling & HMR |
| 4 | Input & text editing |
| 5 | Accessibility |
| 6 | Runtime standard library |
| 7 | Majority style & Tailwind coverage |
| 8 | Developer tools |
| 9 | Application shell & platform integration |
| 10 | React custom renderer |
| 11 | Cross-platform support |
| 12 | 100% style & Tailwind parity |
| 13 | App-owned Rust extensions |

Phases are built in numeric order, and each names its gate.

## Phase 1: Rust host & FFI bridge core (`inca-gpui`)

1. **QuickJS context setup**: use `rquickjs` to spin up a managed QuickJS
   runtime inside the GPUI event loop.
2. **Retained virtual tree**: an in-memory `VirtualNode` structure — see
   [FFI.md](./FFI.md#retained-virtual-tree).
3. **Binding functions** exposed to JS as `globalThis.__inca_native__` — see
   [FFI.md](./FFI.md#binding-functions).
4. **GPUI rendering pipeline**: recursively convert the `VirtualNode` tree into
   GPUI `AnyElement` instances during GPUI's `render()` frame cycle.

## Phase 2: JS core bridge (`incajs`) & Vue 3 custom renderer (`incajs/vue`)

1. **`incajs`** (`packages/core`): a framework-agnostic, typed JS wrapper
   around `globalThis.__inca_native__` (see
   [FFI.md](./FFI.md#binding-functions)), shared by every framework adapter.
2. **`incajs/vue`** (`packages/core/src/vue`): a custom Vue 3 runtime adapter
   using `@vue/runtime-core`'s `createRenderer`, built on `incajs`. It is a
   subpath of the same package as the core.
3. Map Vue node lifecycle methods (`createElement`, `insert`, `remove`,
   `patchProp`) to `incajs`'s calls.
4. A unified mount API, e.g. `createIncaApp(App).mount('#root')`.

## Phase 3: Developer tooling & HMR integration

The JS/TS side owns the `inca` CLI's process orchestration. `@incajs/cli`
(`packages/cli`) is the parent process. It drives a bundler adapter that
holds Vite in-process, and its own `dev-client` spawns the Rust host
(`crates/inca-host`) as a child and bridges dev-server messages over its
stdio. Both modules live inside `@incajs/cli`, since neither is ever imported
on its own.

Two alternatives were rejected: a Rust-primary `crates/inca-cli` that owns
everything, and Rust-primary logic behind a thin npm `bin` wrapper. The
reasons:

- Node already owns every orchestration primitive this needs (Vite's own
  server/watch/restart API, `child_process`, terminal logging), where Rust
  would grow equivalent process/IPC/file-watching plumbing from scratch.
- The host binary ships through npm either way, since app authors aren't
  expected to have a Rust toolchain (Phase 13 is the one exception). A Rust
  CLI would add a second binary to distribute for no gain.
- It keeps `crates/inca-gpui` and the host free of process/IPC concerns.

Phase 3 lands in four numbered stages. Real HMR comes last. A full-reload dev
loop already needs the spawn, teardown and remount skeleton that HMR builds
on. This order makes `dev`, `build` and packaging usable end to end before the
hardest piece starts.

A GitHub Actions CI workflow that runs the required checks lands before 3.1,
so the first multi-package phase is not built without one. Its CD counterpart
lands after 3.3, when there is something to release.

### Phase 3.1: `inca dev` (full reload)

1. **`@incajs/cli`** (`packages/cli`): owns the `inca` commands, resolves
   an app's entry point, and wires the two modules below together
   internally. It holds the `Bundler` contract and injects an
   implementation. Swapping bundlers is a dependency change here.
2. **`adapter/vite`** (`packages/cli/src/adapter/vite`): runs Vite in
   library/watch mode, using `@vitejs/plugin-vue` to compile `.vue` SFCs, and
   announces each rebuild. It never starts Vite's browser dev server. It is
   the only part of the CLI that imports `vite`, and it depends on no
   first-party package. A future `adapter/rspack` would be a sibling module
   in the same package, since neither is ever imported on its own.
3. **`dev-client`** (`packages/cli/src/dev-client`): the Node end of
   [PROTOCOL.md](./PROTOCOL.md) — resolves and launches the host
   binary, supervises the child, and carries messages both ways. It depends
   on no bundler and never parses a routed payload, so Vite's HMR traffic
   (Phase 3.4) rides the same channel as a registered `method` name.
4. **`crates/inca-host`**: the runtime binary. It opens the GPUI window
   and evaluates a bundle in QuickJS. In dev mode it reads newline-delimited
   JSON messages on stdin and re-evaluates the bundle in a fresh engine
   against a reset tree on each reload. Its stdout is the protocol channel,
   and logs go to stderr.
5. **Native root handle**: a binding replacing Phase 2's
   `__INCA_ROOT_ID__` source substitution, so an app's entry point is
   plain code (`createIncaApp(App).mount()`) with no host-injected token
   in it.
6. **Core corrections**: node lifetime (nothing frees a detached node),
   error visibility (a listener's exception reaches nobody, and QuickJS has
   no `console`), and the per-frame cost of wiring every node for input.
   All three are `crates/inca-gpui` gaps that a dev loop running a real app
   continuously makes unavoidable, so they land here rather than after the
   release.

A reload loses component state. Phase 3.4 adds state preservation.

### Phase 3.2: `inca build`

The one-shot production counterpart of 3.1's pipeline, emitting a
production build — potentially multiple files, since a real module loader
now resolves `import`s against disk instead of requiring one self-contained
bundle. Subsumes the per-example `scripts/build.mjs` files Phase 2 Unit iv
hand-rolled.

### Phase 3.3: Application packaging

Pairs a built bundle with a prebuilt host binary into a distributable
application: `.app` on macOS, with each other platform's target following
its support in Phase 11. It also covers the per-platform npm distribution of
those prebuilt hosts. **Design constraint**: keep the host binary swappable,
so Phase 13 can substitute an app-compiled one.

This is the **first release milestone**: once packaging works, the framework
is published as `v0.0.1`, to npm only (`incajs`, `@incajs/cli`, and the
per-platform host packages). The Rust crates stay
`publish = false` — nothing outside this repo depends on them until Phase 13.

### Phase 3.4: HMR

**HMR bridge** (`@incajs/cli`'s `adapter/vite`, beside its Node-side dev
server): a custom `ModuleRunnerTransport` with Vite's stock evaluator against
Vite's Runtime API (`vite/module-runner`). Updated modules are evaluated
inside QuickJS and trigger a GPUI redraw. Component state survives where
Vue's own HMR can: a template-only edit keeps it, and a script edit still
remounts the component. The runner itself runs inside QuickJS. See
[ARCHITECTURE.md](./ARCHITECTURE.md#hmr-delivery) for why, and for why this
is preferred over a hand-rolled HMR protocol. A future `adapter/rspack` would
hold its own Node/QuickJS halves the same way.

Ships **experimental and opt-in**: without the flag, `inca dev` keeps this
phase's full reload unchanged. See [PLAN.md](./PLAN.md#phase-34-hmr)
for the opt-in surface and the prerequisites this needs first.

## Phase 4: Input & text editing (in progress)

Everything here reaches JS through `addEventListener`. The host dispatches
any `(node id, event name)` pair, so a new event is a name the host agrees to
send.

1. **Pointer input**: press, release, move, wheel, enter and leave, with
   propagation control through `stopPropagation`, `stopImmediatePropagation`
   and `preventDefault`. See [FFI.md](./FFI.md#event-dispatch).
2. **Keyboard input**: key press and release with modifiers, and a focus
   model that decides which node receives them: `focusNode` and `blurNode`,
   a click on a focusable node, and the `focus` and `blur` events.
3. **Text editing**: an editable text element, with selection, caret and IME
   composition. This is the largest item. It has no partial version worth
   shipping, since a text field that drops IME composition is unusable in
   Japanese, Chinese and Korean.
4. **Scrolling**: `overflow`, `overflow_x` and `overflow_y` (`visible`,
   `hidden`, `scroll`, `auto`) and wheel scrolling. Nested containers scroll
   innermost first.
5. **Interactive visual state**: hover, active and focus styling mapped onto
   GPUI's own element states, instead of re-derived in JS. Phase 12's
   `hover:` and `focus:` Tailwind variants build on this.
6. **Event payloads**: a listener's callback receives
   `{ type, target, currentTarget, ...payload }`. Each wired input has a name
   and an `EventMask` bit in `crates/inca-gpui`'s `EventKind`, and a new
   input adds a variant and a payload shape with no new plumbing.

## Phase 5: Accessibility (future)

Not started. It waits on Phase 4's focus and text models being stable. A
screen reader reads a focus path, so there is nothing to expose before one
exists.

Electron inherits this layer from Chromium. Incarnative.js renders to the GPU
directly and inherits none of it, so all of it is ours to build. It is
scheduled right after input, since retrofitting an accessibility tree onto a
node vocabulary that grew without one means rewriting that vocabulary.

1. **Semantics on the retained tree**: a node's role, name, value and state,
   carried alongside `style_props` and `attributes`. The `tag_name`
   vocabulary is thin (see [FFI.md](./FFI.md#tag-vocabulary)), so semantics
   are declared on the node.
2. **Platform accessibility APIs**: expose that tree through each platform's
   own API, following what GPUI already supports and filling in the rest.
3. **Keyboard reachability**: every interactive node is reachable and
   operable without a pointer. This is Phase 4's focus model applied
   consistently.
4. **The author-facing surface**: `role`/`aria-*` on a `.vue` template maps
   onto the above, so an author writes what they already write for the web.

## Phase 6: Runtime standard library (future)

Not started. QuickJS provides the language only. Phase 3.1 adds `console`.
Timers, network and filesystem are absent, so an app cannot poll, fetch or
read a file.

Each item is a host binding plus its typed wrapper in `packages/core`,
and each hands a new capability to app code — the FFI safety checklist in
[PLAN.md](./PLAN.md) applies to all of them. They belong in
`crates/inca-jsenv`, which depends on `rquickjs` alone so an
implementation can be swapped for a third-party one.

Check for one before writing any of these. `rquickjs-extra-*` (the rquickjs
org's own: timers, url, os, sqlite) and `llrt_modules` (AWS) both cover part
of this list, and both were pinned to `rquickjs` releases older than this
workspace's when `console` landed, which is why `console` is ours.

1. **Timers**: `setTimeout`/`setInterval` and their `clear` counterparts,
   driven by GPUI's own event loop rather than a second one. This is also
   what pumps the JS job queue between input events, which today is drained
   only when the host has a reason to run JS.
2. **Network**: `fetch` over a Rust HTTP client, and a WebSocket client.
   Both are asynchronous, so promise integration has to cover them.
3. **Filesystem and paths**: reading and writing app data. A desktop app has
   no browser origin to bound it, so what it may reach is a decision this
   item records.
4. **Encoding and crypto**: `TextEncoder`/`TextDecoder`, `crypto`'s random
   sources, `structuredClone`, `URL` — small, standard, and assumed present
   by ordinary npm dependencies.
5. **Engine limits**: a memory ceiling, a stack ceiling, and an interrupt
   handler, so a runaway app stays recoverable and does not freeze the window (see
   [PROTOCOL.md](./PROTOCOL.md#failure-handling)).

## Phase 7: Majority style & Tailwind coverage (future)

Not started. It builds on Phase 3.1's Vite integration: Tailwind's JIT
compiler runs as a build-time step, so it needs a real Vite pipeline to plug
into. Full CSS and Tailwind parity is Phase 12's goal. This phase targets the
structural utility categories that cover most real-world usage and map
cleanly onto GPUI's native styling model:

1. **Native style vocabulary expansion** (`crates/inca-gpui`): close the gaps
   in the style vocabulary. The keys in
   [FFI.md](./FFI.md#style-prop-vocabulary) are the ones the host reads
   today. The gaps are percentage lengths, flex-basis, per-side border width
   and radius, basic box-shadow, font-weight and family, line-height and
   letter-spacing, the `clip` overflow value, `border_style`, further
   align and justify values, and `position` with `z-index`.
   - `position` (`relative`/`absolute`, with `top`/`right`/`bottom`/`left`)
     maps directly onto `gpui`'s own positioning. Small.
   - `gpui` has no z-index: paint order and hit-testing follow tree order,
     so ordering siblings is a sort at spec build. Medium.
   - Lifting an element above everything (a modal, an overlay, the dev error
     panel) goes through `gpui`'s `deferred` priority. It must be checked
     against focus, hover, scrolling and ancestor clipping. Large.
2. **Tailwind class resolver**: Incarnative.js has no CSS engine, so Tailwind
   utilities cannot generate CSS. A Vite plugin, building on Phase 3's
   pipeline, scans `class="..."` usage and maps each recognized utility
   directly to a `setStyle` call.
3. **Scope target: roughly 70–75% of Tailwind's utility classes**:
   layout/flexbox/grid, spacing, sizing, typography basics, solid
   background/text/border colors, borders/radius, basic shadow. Explicitly
   deferred to Phase 12: responsive breakpoint variants (`sm:`/`md:`/...),
   state variants (`hover:`/`focus:`/`group-*`), dark mode,
   animations/transitions, transforms, filters/backdrop-filters, and
   arbitrary bracket values (`w-[137px]`). These need design work, for
   example mapping `hover:` onto GPUI's own interactive element states. A
   style-prop translation does not cover them.

## Phase 8: Developer tools (future)

Not started. It waits on Phase 6, since `@vue/devtools-kit` is ordinary npm
code and expects a runtime to live in.

The goal is the real Vue DevTools. `vuejs/devtools` splits into a collector
and a UI. The collector is `@vue/devtools-kit`, which hooks
`globalThis.__VUE_DEVTOOLS_GLOBAL_HOOK__`. `@vue/runtime-core` fires that
hook from `createRenderer`, so this project's renderer is already wired for
it. The UI is `@vue/devtools-client`, itself a Vue app. Only the collector
has to run where the app runs.

1. **A dev-only engine.** A second QuickJS engine, the host's own, drawing
   into a second root stacked over the app's. It outlives the reload that
   replaces the app's engine, leaves the app's tree alone, and can still
   draw when the first bundle never loaded. Phase 3.1's failure panel needs
   that too.
2. **The collector beside the app.** `@vue/devtools-kit` in the app's own
   engine, with the dev build's `__VUE_PROD_DEVTOOLS__` on. It forwards over
   the channel [PROTOCOL.md](./PROTOCOL.md) already carries, nested in
   `params` the way Vite's frames are.
3. **The UI in a browser, first.** `@incajs/cli` serves
   `@vue/devtools-client` and bridges it to that channel. This is how Nuxt
   DevTools works, and it asks nothing of the style vocabulary.
4. **The UI in the window, eventually.** `@vue/devtools-overlay` mounted
   through this project's own renderer and floating over the app — Phase 12
   is where the style vocabulary can carry it.

## Phase 9: Application shell & platform integration (future)

Not started. It builds on Phase 3.3's packaging: these are the APIs a
packaged application calls, and several have no meaning until there is one.

An Incarnative.js app is one window that it cannot address. Everything an app
does around its content lives in this phase.

1. **Windows**: title, size and position, minimize, maximize and fullscreen,
   close behaviour, and more than one window per app. The host's
   one-session-per-process shape does not express that today.
2. **Native menus**: an application menu bar and context menus, with their
   keyboard shortcuts. The host already owns a `Quit` item and its shortcut,
   and an app cannot remove them (see [FFI.md](./FFI.md#application-menu)).
   What an app adds beside it, and where those items come from, is this
   item's work. The candidates are `inca.config.ts`, a JS binding and an SFC.
3. **Dialogs**: file open/save and message boxes, drawn by the platform.
4. **System integration**: clipboard, notifications, a tray icon, and
   handing a URL or file to whatever the platform opens it with.
5. **App lifecycle**: hooks for a reload and for shutdown, window close,
   focus and blur, and the platform's own quit request.

## Phase 10: React custom renderer (future)

Not started. It waits on Vue 3 support (Phases 1–3) being stable. It adds
`incajs/react` as a subpath alongside `incajs/vue`, in the same `incajs`
package. The renderer uses `react-reconciler` and is built on the same core
(see Phase 2). `@vitejs/plugin-react` compiles JSX and TSX and handles HMR.

## Phase 11: Cross-platform support (future)

Not started. It waits on the core Rust host design (Phases 1–2) being stable,
for the same reason as Phase 10. macOS is the primary development target until
then.

1. **Linux**: full support has two parts. The rendering is verified directly
   on a Linux display, where today it is only seen indirectly through the
   devcontainer forwarded to a Mac. And the packaged app starts with no system
   libraries installed by the user, where today the host needs the shared
   libraries listed in the install guide.
2. **Windows**: the `gpui_windows` platform backend.

## Phase 12: 100% style & Tailwind parity (future)

Not started. It waits on cross-platform support (Phase 11) being stable. It
closes the gap Phase 7 deferred, so that every Tailwind utility class a Vue
(and later React) author reaches for resolves to a correct native rendering,
beyond the common ones Phase 7 covers. It is the final styling milestone.

- Full CSS-property parity in the native style vocabulary and render
  pipeline: animations and transitions, transforms, filters, gradients and
  arbitrary values.
- State variants mapped onto GPUI's own interactive element states: hover,
  focus and active.
- Responsive breakpoints. There is no browser viewport concept here, so this
  needs its own window-size-aware style-resolution design.
- Dark mode.

The acceptance test is `@vue/devtools-overlay` running in an app's own window
through this renderer. It pulls in `shiki`, `vue-virtual-scroller` and
`focus-trap`, so the phase is finished when a real third-party Vue app
renders correctly.

## Phase 13: App-owned Rust extensions (future)

Not started. It waits on Phase 3.3's packaging and Phase 12's styling being
stable. Every phase before this one assumes that app authors write only JS
and TS and consume a prebuilt host binary. This phase adds the opt-in case
where an app moves its own heavy work (compute, native I/O) into Rust and
still ships as a single application:

1. **App-owned host build**: an app that carries its own Rust crate gets a
   host compiled from source with that crate linked in, in place of the
   prebuilt binary. This is the swappability Phase 3.3 is required to
   preserve. Apps without one keep needing no Rust toolchain.
2. **Extension binding surface**: a stable way for app-owned Rust code to
   register its own functions alongside `__inca_native__` (see
   [FFI.md](./FFI.md#binding-functions)), instead of patching the host's own
   bindings.
3. **MSRV verification**: `rust-version` is held equal to the pinned
   toolchain while these crates have no consumers outside this repo. Once
   app crates compile against them it drops to a real floor, checked by its
   own CI job.

## Known gaps, not yet scheduled

Items that need a decision before they can join a phase.

- **Release engineering beyond packaging**: auto-update, code signing and
  notarization. Phase 3.3 produces an application with no update or trust path.
- **Crash reporting**: `inca dev` reports a host panic to the client, but a
  packaged app's panic leaves nothing behind for the person whose app died.
  Its cheap half is a log file or dialog written from the existing panic hook.
  A native fault under it needs an out-of-process collector, which is separate
  work.
- **A JS debugger**: QuickJS ships no inspector protocol, and nothing maps a
  running frame back to a `.vue` source line. Today a bundle's failure is a
  message and a stack, and nothing steps through it.
- **The security model for untrusted code**: every binding this roadmap adds
  is reachable by anything in the bundle, dependencies included. Whether that
  is acceptable, and what would constrain it, is unanswered.
