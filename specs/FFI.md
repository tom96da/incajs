<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Host bridge (FFI) reference

The function surface exposed to JS as `globalThis.__inca_native__`, bound
into the QuickJS context by the Rust host via `rquickjs`. See
[AGENTS.md](../AGENTS.md#status) for how much of this is built. The
tag/style vocabulary below is deliberately incomplete by design and grows as
real usage needs more of it — update this file whenever a binding, tag, or
style prop actually lands.

It's called "FFI" for the calling-convention style (JS calling into Rust
functions with typed arguments), not a real C ABI or cross-process boundary —
JS and Rust share one process.

## Retained virtual tree

The Rust host keeps an in-memory node structure (`VirtualNode`) that mirrors
the custom renderer's output. Each node has:

| Field | Type | Purpose |
| --- | --- | --- |
| `id` | `u32` | Stable handle returned to JS, used in all subsequent calls. |
| `tag_name` | `String` | Element kind, used to pick a GPUI element builder. |
| `style_props` | map | Layout/paint style properties. |
| `attributes` | map | Non-style attributes/props. |
| `parent` | `Option<u32>` | The node this one is attached to, if any. |
| `children` | `Vec<u32>` | Ordered child node handles. |

**A node has at most one parent.** `appendChild`/`insertBefore` detach the
child from wherever it was, so the same call both attaches and moves, and an
attachment that would make a node its own ancestor throws instead. A walk
down the tree therefore always terminates, which the render path relies on.

**Nodes are freed only by `destroyNode`,** which frees the whole subtree.
Detaching with `removeChild` keeps the node alive for re-attachment; a caller
that drops a subtree without destroying it leaks every node in it, and every
event listener registered on them.

### Tag vocabulary (v1)

Implemented by `crates/inca-gpui/src/element.rs` (Unit v). Only two
kinds exist so far — there's no per-tag dispatch table yet, since there's
exactly one container builder to pick from until a real second element kind
is designed:

| `tag_name` | Maps to |
| --- | --- |
| `"text"` | A leaf. Content comes from the `"value"` string attribute (missing/non-string → empty content, never a panic). |
| anything else | A generic styled container (a GPUI `div()`). |

### Style prop vocabulary (v1)

Also implemented by `element.rs`. This is a deliberately small,
initial set — exactly what's needed to express `examples/gpui/hello_world.rs`'s
flex-box shapes and solid fills, not a full CSS surface. Unrecognized keys
and malformed enum-string values are silently ignored (forward-compatible,
never a panic) — this is a rendering path, not a JS call boundary, so
there's no channel to raise a catchable exception through.

| Key | Value | Maps to |
| --- | --- | --- |
| `display` | `"flex"`\|`"block"`\|`"grid"`\|`"none"` | `Style::display` |
| `flex_direction` | `"row"`\|`"column"`\|`"row_reverse"`\|`"column_reverse"` | `flex_direction` |
| `justify_content` / `align_items` | `"start"`\|`"end"`\|`"center"`\|`"stretch"` | resp. fields |
| `gap` | number (px) | `gap.width` & `gap.height` (uniform) |
| `width` / `height` | number (px) or `"auto"` | `size.width` / `size.height` |
| `border_width` | number (px) | all four `border_widths.*` (uniform) |
| `background` / `border_color` / `text_color` | number (hex `0xRRGGBB`) or string (`"#rrggbb"`/`"#rgb"`) | `Fill`/`Hsla` |
| `corner_radius` | number (px) | all four `corner_radii.*` (uniform) |
| `text_size` | number (px) | `text.font_size` |

`text_color`/`text_size` only apply to containers — text leaves cascade
their style from an ancestor container, exactly like GPUI's own
`.text_color()`/`.text_size()`; there's no separate per-leaf text styling.

Deliberately deferred, not yet implemented: percentage lengths, min/max
size, margin/padding, flex-grow/shrink/basis, per-side border/corner
values, box-shadow, `border_style` (solid vs. dashed — GPUI's own
`Style::border_style`, distinct from `border_width`/`border_color`; unset
always renders solid, GPUI's default), and additional align/justify
variants beyond the four above.

## Binding functions

| Function | Signature | Purpose |
| --- | --- | --- |
| `rootNodeId` | `() => number` | Return the host-allocated root container's id — the node a mounting app attaches itself under. Allocated with the tree, so it always resolves. |
| `createNode` | `(tag: string) => number` | Allocate a `VirtualNode`, return its id. |
| `appendChild` | `(parentId: number, childId: number) => void` | Attach a child node at the end of `parentId`'s children. Thin wrapper over `insertBefore` with no anchor. |
| `insertBefore` | `(parentId: number, childId: number, anchorId: number \| null) => void` | Attach a child node before `anchorId` (or at the end if `null`). If `anchorId` names a real node that isn't currently a child of `parentId`, falls back to appending at the end; only a wholly unknown `anchorId` throws. |
| `removeChild` | `(parentId: number, childId: number) => void` | Detach a child node. |
| `setAttribute` | `(nodeId: number, key: string, value: any) => void` | Set a non-style attribute prop. |
| `setStyle` | `(nodeId: number, key: string, value: any) => void` | Set a style prop — the only JS-reachable way to touch `style_props`; `setAttribute` writes to the separate `attributes` map instead. |
| `addEventListener` | `(nodeId: number, event: string, callbackId: number) => void` | Register a JS callback for a native input event. Distinct ids on one `(nodeId, event)` stack and all of them are dispatched, as in the DOM; re-registering an id already there is a no-op. |
| `removeEventListener` | `(nodeId: number, event: string, callbackId: number) => boolean` | Drop one registration, reporting whether it was there. Never throws — a node destroyed first is the normal teardown race, not an error. |
| `destroyNode` | `(nodeId: number) => number[]` | Free `nodeId` and its whole subtree, and return every `callbackId` that was registered anywhere in it, so the caller can drop the JS functions those ids name. Destroying an already-destroyed or unknown id returns `[]`. Destroying the root throws — it belongs to the host. |

On each GPUI `render()` frame cycle, the host recursively converts the
`VirtualNode` tree into GPUI `AnyElement` instances. A node is wired for
input only while something is registered on it: GPUI inserts a hitbox for
any element carrying a click listener, so wiring a whole tree would cost a
hitbox and two mouse listeners per node per frame, and hit-test all of them
on every pointer move.

### Event dispatch

Implemented by `render_tree_with_events`/`build_element_with_events`
(`crates/inca-gpui/src/element.rs`, Unit vi), which wire every container's
`"click"`/`"mousedown"`/`"mouseup"`/`"mousemove"`/`"wheel"` to an
`EventDispatcher` (`crates/inca-bridge/src/dispatch.rs`), which looks up
and calls the JS callbacks registered for `(nodeId, event)` via
`addEventListener`, then requests a redraw.

A callback receives one argument, shared across every callback on one node
for one event: an object shaped `{ type, target, currentTarget, ...payload
}` plus `stopPropagation`/`stopImmediatePropagation`/`preventDefault`
methods. `type` is the event's name. `target`/`currentTarget` are both the
node this call is dispatching for — every container with a listener gets a
hitbox, but one without a listener doesn't, so the node a pointer visually
lands on isn't always known; `currentTarget` is exact, `target` will read
the same until something can compute it precisely (tracked in
[BACKLOG.md](./BACKLOG.md)). Further fields depend on the event kind —
`"click"` carries none of its own; `"mousedown"`/`"mouseup"`/`"mousemove"`
carry `clientX`, `clientY`, `button`, `buttons`, `detail`, and
`ctrlKey`/`shiftKey`/`altKey`/`metaKey`, DOM-`MouseEvent`-named (`platform`
becomes `metaKey`; GPUI's `function` modifier has no DOM counterpart and is
dropped). `buttons` tracks every button currently held (`EventDispatcher`
keeps this state across events — GPUI's own mouse events carry only the
one button each is about), not just the button `button` names.

`"wheel"` carries the same fields as `"mousedown"`/`"mouseup"`/
`"mousemove"` plus `deltaX`, `deltaY`, `deltaZ`, `deltaMode` — DOM's
`WheelEvent` extends `MouseEvent`. `deltaMode` is `0`
(`DOM_DELTA_PIXEL`) or `1` (`DOM_DELTA_LINE`); GPUI has no equivalent of
`DOM_DELTA_PAGE` (`2`). `deltaZ` is always `0` — GPUI carries no Z-axis
scroll.

`stopImmediatePropagation()` stops the remaining callbacks *on that node*.
`stopPropagation()`/`preventDefault()` are read back once every callback on
the node has run, and forwarded into GPUI's own dispatch — GPUI already
bubbles from the node a pointer hit outward through its ancestors the same
way the DOM does, so `stopPropagation()` keeps ancestor listeners for the
same event from firing. `preventDefault()` reaches only what GPUI itself
uses it for (`Window::prevent_default`'s doc comment) — narrower than the
DOM's. `"click"` fires *before* `"mouseup"` on the same node: GPUI
synthesizes clicks from its own mouse-down/mouse-up bookkeeping, registered
after this crate's own `mouseup` wiring, and bubble-phase listeners run in
reverse registration order.

`crates/inca-gpui`'s `EventKind` enumerates every native event kind a node
can be wired for, and pairs each with its name and its `EventMask` bit.
A node's spec carries one mask covering everything it listens for.
Extending the wired input vocabulary means adding a variant to `EventKind`
and a payload variant to `EventPayload` — same "deliberately incomplete"
framing as the style vocabulary.

`EventMask` covers only the fixed vocabulary of native input `inca-gpui`
wires per-frame. A dynamically-named event — a host-lifecycle event or a
menu item's activation (`menu:<id>`) — reaches JS through the same
`addEventListener`/`EventDispatcher::dispatch` path; `EventMask` doesn't
cover it.

`addEventListener` itself is unchanged and needs no thread-safe/cross-thread
callback machinery: Incarnative.js's embedded QuickJS and the GPUI event loop
already share one process and are driven synchronously (see
`crates/inca-jsenv/src/engine.rs`'s `Context::with`), unlike an architecture
where JS runs in a separate runtime that loads a native addon (JS and the
native UI layer on different threads/processes) — confirmed, not just
assumed, by `crates/inca-bridge/tests/event_dispatch.rs`.

`EventListeners` (`crates/inca-bridge/src/bindings.rs`) only ever stores the
plain `u32` `callbackId` it's given — never an
`rquickjs::Value`/`Function`/`Persistent<T>`, per the FFI safety
checklist below. The real JS function has to live somewhere, so the
convention is: **the caller stores it itself**, at
`globalThis.__inca_callbacks__[callbackId]`, before calling
`addEventListener` with that id. `EventDispatcher::dispatch` looks the real
function up fresh inside one `Engine::with` call and drops it before that
call returns — it never crosses into a long-lived Rust struct. A missing
`__inca_callbacks__` entry, or one that isn't a function, is a stale id and
is skipped. A callback that *throws* is reported through the host's error
reporter and the remaining callbacks still run: one bad listener must take
down neither the host nor its siblings.

That is why `removeEventListener` and `destroyNode` report the ids they
dropped: each side holds half of a registration, and only the caller can free
the JS half.

### App lifecycle surface (decided, not yet dispatched)

Settled ahead of `@incajs/cli`'s `inca dev` needing it, per
[PROTOCOL.md](./PROTOCOL.md)'s own note that an app lifecycle hook is "a name
the host agrees to dispatch, not a new binding." Recorded here so a future
unit implements this rather than deciding it again:

- **Moments**: exactly the two the dev protocol itself creates — a reload
  about to discard the current session, and process exit (`shutdown`).
  Nothing else; window close/focus/a platform quit request is Phase 9.
- **Mechanism**: no new binding. A named event (e.g. `"beforeReload"`,
  `"beforeUnmount"`) dispatched on `rootNodeId()` through the existing
  `addEventListener`/`EventDispatcher` path above.
- **Cancellation**: observe-only. By the time a reload's hook would fire,
  the new bundle has already loaded successfully — `inca-host`'s
  `reload()` only swaps sessions after that succeeds — so there is
  nothing left to veto. Shutdown can't be blocked indefinitely either.
- **What a handler may await**: only already-settled microtasks. The job
  queue drains once right after the hook fires, the same as every
  existing `drain_jobs_and_refresh` call site, and nothing pumps it again
  afterward — QuickJS has no timers or I/O to resume it, so awaiting a
  timer or `fetch` would hang forever, not fail loudly.
- **Wrapping**: `packages/core` should expose named helpers (e.g.
  `onBeforeReload`/`onBeforeUnmount`) over the raw `addEventListener`
  call, so the event-name strings never become app-facing API.

Nothing dispatches either event yet — wiring `crates/inca-host` to
actually fire them is separate, future work.

### Application menu (host-owned, one item)

`crates/inca-host` sets the menu bar (`src/menu.rs`): one top-level menu
titled after the app, holding `Quit`, bound to `secondary-q` — `cmd-q` on
macOS and `ctrl-q` elsewhere. macOS reads a menu item's key equivalent from
the keymap, so the shortcut shown beside the item comes from that binding,
and it titles the application menu from the `.app` bundle's
`CFBundleName`. macOS is the only platform that draws a menu bar today;
Linux and Windows keep what `set_menus` was given.

`Quit` is the host's. An app can neither remove nor rebind it, so every app
has a way to quit.

An item an app defines reaches JS through the same path a lifecycle hook
does, with no new binding: its activation is dispatched on `rootNodeId()`
as `menu:<id>`, `<id>` being whatever the app called the item, so an item
`save` arrives as `("menu:save")`. A GPUI action is a type, so an app's own
id travels in one action struct with a `SharedString` field (`Action`
derive, `#[action(no_json)]`), constructed per item.

**Nothing supplies app-defined items yet.** Where they come from —
`inca.config.ts`, a JS binding, or an SFC — is undecided, and settling it
changes only what produces the `Vec<Menu>` that `menu::install` takes.
