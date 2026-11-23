<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Modules

The duty of each crate and package, the layers they form and the edges allowed between them.

## Terms

- **Module**: a crate under `crates/` or an npm package under `packages/`. A Rust `mod` is a module
  of its crate and is called a Rust module here.
- **Layer**: a set of modules that may depend on the layers below it and on nothing above it. The
  modules of one layer do not depend on each other unless this file names the edge.
- **Edge**: a dependency of one module on another. `Cargo.toml` and `package.json` record every
  edge. An edge this file does not allow is a defect.
- **Seam**: a trait or an injected interface that a module owns and a higher module implements or
  passes in. A lower module reaches a higher one only through a seam.

## Layers of the host

```
                              Node   ┆   Rust
┌─────────────────────────────────┐  ┆  ┌───────────────────────────────────────────────┐
│ @incajs/cli                     │──┆─▶│ inca-host                                     │  layer 3
└─────────────────────────────────┘  ┆  │                                               │
                                     ┆  │                                               │
┌──────────────────┬──────────────┐  ┆  ├────────────────────────┬──────────────────────┤
│ App frontend     │  incajs      │◀─┆─▶│ inca-bridge            │ inca-window          │  layer 2
└──────────────────┴──────────────┘  ┆  ├────────────────────────┼──────────────────────┤
                           QuickJS   ┆  │                        │ inca-gpui            │  layer 1
                                     ┆  ├────────────────────────┼──────────────────────┤
                                     ┆  │ inca-jsenv             │ inca-document        │  layer 0
                                     ┆  │                        │ inca-style           │
                                     ┆  │                        │ inca-node            │
                                     ┆  └────────────────────────┴──────────────────────┘
```

- `inca-node`, `inca-style` and `inca-document` form the document model. Inside layer 0 they stack
  in that order: `inca-style` depends on `inca-node`, and `inca-document` on both. `inca-jsenv`
  depends on none of them.
- `inca-window` adapts the document model to `gpui`. `inca-bridge` adapts it to the JS engine. The
  two adapters sit side by side and know nothing of each other.
- `inca-host` joins the adapters and is the only module that knows both.
- The Rust crates are agnostic to the frontend framework and the bundler. A script reaches them
  through the natives of `inca-bridge`, and the CLI reaches `inca-host` through the dev protocol.

## Edges and external dependencies

| Module | May depend on | Main external dependencies it adds |
| --- | --- | --- |
| `inca-node` | none | `html5ever` (markup parsing) |
| `inca-style` | `inca-node` | `cssparser`, `selectors` |
| `inca-document` | `inca-node`, `inca-style` | `unicode-segmentation` |
| `inca-jsenv` | none | `rquickjs` |
| `inca-gpui` | the document model | `gpui` |
| `inca-window` | the document model, `inca-gpui` | none |
| `inca-bridge` | the document model, `inca-jsenv` | none |
| `inca-host` | every crate | `gpui_platform` |

- `rquickjs` sits only in `inca-jsenv`, `inca-bridge` and `inca-host`.
- `gpui` sits only in `inca-gpui`, `inca-window` and `inca-host`.
- An external dependency enters the lowest module whose duty needs it.
- `inca-test-macros` is a dev-dependency of any crate and never a normal one.

## Crates

### `inca-node`

**Duty**: the retained node tree and the data stored on each node.

**Owns**:
- node kinds and the tag vocabulary;
- the parent and child links, and insertion and removal with their checks. A removal returns the
  list of the removed nodes: the id, whether it was attached, and the node before the subtree;
- node lifetime: the hold and the sweep;
- attributes, the class list and the dataset, the style props as stored data and the style key
  vocabulary;
- character data, templates and their content, and cloning;
- markup parsing and serialization;
- the interaction state that selectors read;
- the text rules of form controls: typed text, pasted text and the stored value;
- a change counter that lets a cache built from the tree detect that it is stale.

**Knows nothing of**: the cascade, events, layout, gpui and JS.

### `inca-style`

**Duty**: everything that turns stored style and style sheets into the computed style of a node.

**Owns**:
- parsing of style values, inline style text and its sync with the `style` attribute;
- style sheets, selector parsing and matching;
- the cascade and its cache;
- selector matching against the drawn interaction state, or with the focus a caller passes;
- the element spec that layout reads;
- the scroll container and wheel axis rules;
- the warning text for a dropped style.

**Knows nothing of**: events, the focus model, layout boxes, gpui and JS.

### `inca-document`

**Duty**: the operations that the standards define for the document, and the models that span the
tree and its style.

**Owns**:
- the operations that the standards define for the document, which a caller runs on it;
- the document state that joins the tree and its style. It is read through the document and changed
  only through its operations, each running the follow-up steps the change needs;
- applying the list of removed nodes that `inca-node` returns in one step;
- semantics: ARIA, `tabindex`, focusability and Tab order;
- the focus model: the focused node, the focus plan, the queued requests, the reveals that wait for
  a box and the drawn focus that selectors read;
- the pointer capture request that waits for the next pointer event;
- the event model: the event vocabulary and payloads as plain data, event targets, the listener
  registry, the signals and the dispatch algorithm, and key codes;
- the text-control model: value, selection, composition and edit operations;
- the style warnings: the sink and the rules for what it receives;
- box and scroll geometry as plain numbers.

**Seams it owns**:
- `View`: the layout boxes, scroll offsets and scrolling of the drawn frame. `inca-window`
  implements it.
- `Invoke`: calls a registered listener by its id and reports the stop and cancel flags.
  `inca-bridge` implements it.
- a warning sink that receives each warning line, set through the document.

**Knows nothing of**: gpui and JS.

### `inca-jsenv`

**Duty**: the JS engine and the Web built-ins a script expects from its environment.

**Owns**:
- the engine, with its job queue and the queues of timers and frame callbacks. A callback table
  keeps the functions that a script registered, under ids, so Rust holds no JS value;
- the module loader, source maps and value inspection;
- `console` with its formatting and the decoration of each log level;
- the Web built-ins: `URL`, encoding, `crypto`, `structuredClone`, `performance`, `DOMRect` and
  `DOMRectReadOnly`;
- the vault: one object per realm, with no prototype, in which host code keeps JS values between
  calls under plain keys. A script cannot reach it. The global that opens it returns `undefined`
  outside a call of host code.

The `quickjs` Rust module holds every `unsafe` block written in this repository, wraps `rquickjs`
in its own style and imports nothing from the other crates.

**Seams it owns**:
- the console output, which `inca-host` passes in with the color decision;
- a writer handle that lets Rust code write a line through the same console. It enters the console
  before the decoration stage, so a line from Rust and a line from `console` get the same
  decoration;
- `ErrorReporter`: receives each failure of the engine's work. `inca-bridge` passes it in, and the
  default writes one line on stderr.

**Knows nothing of**: the document model, gpui and every crate above it.

### `inca-gpui`

**Duty**: drawing the tree with `gpui` and turning platform input into the plain payloads of the
document model.

**Owns**:
- the element builder;
- styles applied to `gpui`;
- scrollbars;
- text input drawing and its layout of stops and caret;
- recording each drawn box as plain geometry;
- key names read from platform keystrokes.

**Seams it owns**:
- `EventSink`: receives each input and each recorded box. `inca-window` implements it.

**Knows nothing of**: JS, the event semantics behind `EventSink` and every crate above it.

### `inca-window`

**Duty**: the behaviour of a window: the input it receives turned into events and default
actions, and the state that lives with a `gpui` window.

**Owns**:
- hover;
- pointer capture and held buttons: applying the capture request of the document at the next
  pointer event, and the active capture;
- clicks and their counts;
- key default actions;
- text edit defaults and the composition of the input method;
- the activation of the window and the focus and blur it causes;
- modifier-key events and the wheel step;
- filling the payload fields of events that come from window state;
- applying the focus plan: the focus handle of each node and the platform focus. Every turn starts
  from a fresh snapshot of the platform focus;
- scroll handles, applying the queued reveals, and `scroll` events;
- the frame order and the schedule of timers and frame callbacks.

**Seams**:
- It implements `EventSink` and `View`.
- `Script`: runs the callbacks tied to events and timers. `inca-host` provides it.

**Knows nothing of**: JS, `rquickjs` and `inca-bridge`.

### `inca-bridge`

**Duty**: the JS side of the document model: the JS values that stand for its nodes and events, and
the natives and hooks that connect a script to it.

**Owns**:
- the `__inca_native__` natives;
- the interface prototypes and their members;
- the native classes with their data;
- the event objects;
- the calls into the hooks on `globalThis`;
- the turns of the engine.

**Seams**:
- It implements `Invoke`.
- It reaches window state only through `View`.

**Knows nothing of**: gpui and `inca-window`.

**Placement rule**. Code goes by what it touches:
- JS values that stand for data of the document model, or a call of its operations for a script:
  they go here. A native converts its arguments, calls one operation of a lower crate and converts
  the result or the error.
- JS values that stand for no data of the document model: they go to `inca-jsenv`.
- No JS value: it goes below. The document algorithms, checks, tables and state go to the document
  model, and window behaviour goes to `inca-window`.

The Rust modules of `inca-bridge` follow the interfaces. One Rust module holds the natives and
the members of one interface.

### `inca-host`

**Duty**: the runtime binary and the composition root.

**Owns**:
- the process entry and bundle discovery;
- the window and its menu;
- the app session that installs the engine, the bridge and the window;
- the console destination;
- the dev protocol;
- the automation requests, the snapshot and the test `Harness`;
- the settings an app declares and the run log of a packaged app.

**Seams**:
- It passes `View` to the bridge and `Script` to the window, through an adapter over the bridge.
- It passes the console writer to the warning sink of the document model.

### `inca-test-macros`

**Duty**: the test attribute macros. It is a dev-dependency only and never part of the `inca-host`
build.

## Packages

### `incajs`

**Duty**: the core JS package. It holds:
- the host-call wrapper, the only code that calls the natives;
- the element handles, the document and window objects, and the event targets;
- the Vue renderer under the `incajs/vue` subpath.

**Edges**:
- The core features import no framework.
- The renderer reaches the core through the injected `rendererCore`.
- Vue is an optional peer.
- The package knows no bundler and no CLI.

### `@incajs/cli`

**Duty**: the `inca` commands, the dev-protocol client that runs the host as a child, the config
loader and the Vite adapter.

**Edges**:
- An adapter reaches the CLI through the injected `adapterCore`.
- Only the Vite adapter imports Vite.
- One file names `incajs` and the renderer module.
- The CLI depends on no Rust crate. It runs the host binary of the platform package.

### Platform packages

The packages under `npm/` each carry the host binary of one platform and hold no code.

## Tests that cross crates

- **Wiring tests** live in `inca-host` `tests/`. They check one contract each where the JS side and
  the window meet.
- **`tests/`** (`inca-tests` and the e2e package) holds the tests that need both Rust and
  TypeScript, and every test that spawns the real host binary.
