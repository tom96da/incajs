<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Host bridge (FFI) reference

The function surface exposed to JS as `globalThis.__inca_native__`, bound
into the QuickJS context by the Rust host via `rquickjs`. JS and Rust run in
one process, and a call is an in-process function call with typed arguments.
No C ABI or IPC is involved.

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

- A node has at most one parent.
- `appendChild` and `insertBefore` detach the child first, so they also move
  a node.
- Making a node its own ancestor throws. A `childId` naming the root, which
  belongs to the host, throws too.
- A `nodeId` that is not a finite whole number in the `u32` range throws a
  `TypeError`.
- Only `destroyNode` frees nodes, and it frees the whole subtree.
  `removeChild` keeps the node alive for re-attachment.

### Tag vocabulary

Implemented in `crates/inca-gpui/src/element.rs`. There are three kinds:

| `tag_name` | Maps to |
| --- | --- |
| `"text"` | Content is its `"value"` string attribute (missing/non-string → empty, never a panic) followed by its descendants' text in child order. Descendants are not rendered as separate elements. |
| `"button"` | A styled container. The host reads its `tabindex` with a default of 0. [Events](../docs/reference/events.md#focus) covers its key clicks and `disabled` presses. |
| anything else | A generic styled container (a GPUI `div()`). |

### Style prop vocabulary

Implemented in `element.rs`. An unrecognized key and an invalid color each
print one warning line on stderr every time a style sets them. A malformed
enum string reads as unset and prints nothing. The table lists every key the
host reads. `set_style` and `remove_style` map the camelCase and kebab-case
spellings of a key to its snake_case form.

| Key | Value | Maps to |
| --- | --- | --- |
| `display` | `"flex"`\|`"block"`\|`"grid"`\|`"none"` | `Style::display` |
| `flex_direction` | `"row"`\|`"column"`\|`"row_reverse"`\|`"column_reverse"` | `flex_direction` |
| `justify_content` / `align_items` | `"start"`\|`"end"`\|`"center"`\|`"stretch"` | resp. fields |
| `gap` | number (px) | `gap.width` & `gap.height` (uniform) |
| `width` / `height` | number (px) or `"auto"` | `size.width` / `size.height` |
| `border_width` | number (px) | all four `border_widths.*` (uniform) |
| `background` / `border_color` / `text_color` | number from `0` to `0xffffff`, or string (`"#rrggbb"`/`"#rgb"`); any other value warns | `Fill`/`Hsla` |
| `corner_radius` | number (px) | all four `corner_radii.*` (uniform) |
| `text_size` | number (px) | `text.font_size` |
| `overflow` / `overflow_x` / `overflow_y` | `"visible"`\|`"hidden"`\|`"scroll"`\|`"auto"` (= `"scroll"`) | `overflow.x` / `overflow.y`; an axis key beats `overflow` |
| `padding` (+ `_x`/`_y`/`_top`/`_right`/`_bottom`/`_left`) | number (px, `>= 0`) | `padding.*`; side beats axis beats shorthand, a wrong-typed value falls through |
| `margin` (+ the same suffixes) | number (px, negatives allowed) or `"auto"` | `margin.*`, same precedence |
| `flex_grow` / `flex_shrink` | number `>= 0` | `flex_grow` / `flex_shrink` |
| `opacity` | number, clamped to `0..=1` (`NaN` ignored) | `opacity` |
| `min_width` / `min_height` / `max_width` / `max_height` | number (px) or `"auto"` | `min_size.*` / `max_size.*` |

`text_color` and `text_size` apply to containers. A text leaf takes them from
the nearest ancestor container.

## Binding functions

| Function | Signature | Purpose |
| --- | --- | --- |
| `rootNodeId` | `() => number` | Return the id of the host-allocated root container, the node a mounting app attaches itself under. |
| `createNode` | `(tag: string) => number` | Allocate a `VirtualNode`, return its id. Throws once every id has been used. |
| `appendChild` | `(parentId: number, childId: number) => void` | Attach a child at the end of `parentId`'s children. Same as `insertBefore` with no anchor. |
| `insertBefore` | `(parentId: number, childId: number, anchorId: number \| null) => void` | Attach a child before `anchorId`, or at the end if `null`. |
| `removeChild` | `(parentId: number, childId: number) => void` | Detach a child node. |
| `setAttribute` | `(nodeId: number, key: string, value: string \| number \| boolean) => void` | Set a non-style attribute prop. |
| `removeAttribute` | `(nodeId: number, key: string) => void` | Remove a non-style attribute prop. Removing a key that isn't set does nothing. |
| `setStyle` | `(nodeId: number, key: string, value: string \| number \| boolean) => void` | Set a style prop. |
| `removeStyle` | `(nodeId: number, key: string) => void` | Remove a style prop so it renders as if never set. Removing a key that isn't set does nothing. |
| `addEventListener` | `(nodeId: number, event: string, callbackId: number) => void` | Register a callback for an event. Ids stack per `(nodeId, event)`, see [Registering](#registering). |
| `removeEventListener` | `(nodeId: number, event: string, callbackId: number) => boolean` | Drop one registration and return whether it was there. An unknown node returns `false`. |
| `destroyNode` | `(nodeId: number) => number[]` | Free `nodeId` and its subtree, and return every `callbackId` registered in it. An unknown id returns `[]`. Destroying the root throws. Also clears the focus state of the destroyed nodes. |
| `focusNode` | `(nodeId: number) => void` | Request focus for `nodeId`. Takes effect next frame. A node with a `tabindex`, and only that node, takes focus. |
| `blurNode` | `(nodeId: number) => void` | Request that `nodeId` lose focus. Takes effect next frame, and does nothing if `nodeId` is not the focused node by then. |

Shared throw rules, all as a `TypeError`:

- A malformed `nodeId` or `callbackId` throws.
- `setAttribute` and `setStyle` throw for a `value` that is not a string, number or boolean.
- `addEventListener`, `setAttribute`, `removeAttribute`, `setStyle`, `removeStyle` and `appendChild` throw for an unknown node. `insertBefore` throws for an unknown parent, child or anchor.
- `removeChild` throws for an unknown `parentId` and ignores an unknown or unattached `childId`.

`insertBefore` and `appendChild`:

- If `anchorId` equals `childId` and the child is already under `parentId`,
  nothing changes.
- If `anchorId` is a real node that is not a child of `parentId`, the child
  is appended at the end.
- An `anchorId` that names no node throws.
- A `childId` naming the root throws.

The host converts the `VirtualNode` tree into GPUI elements on each frame. A
node is wired for input only while a listener is registered on it.

### Event dispatch

The host dispatches native input to the JS callbacks registered for a node
and event name. Payloads and author-facing behaviour are in
[events.md](../docs/reference/events.md).

#### Registering

- `addEventListener`/`removeEventListener` are in the binding table above.
  Distinct `callbackId`s stack on one `(nodeId, event)`. Adding an id again
  is a no-op.
- The caller stores the function at `globalThis.__inca_callbacks__[callbackId]`
  before registering. The host stores only the `u32` id.
- A missing or non-function entry is skipped.
- A throwing callback, or a promise rejected with no handler, goes to the
  host's error reporter. Sibling callbacks still run.
- `removeEventListener` and `destroyNode` return the dropped ids, because
  only the caller can free the JS half.
- A dynamically-named event, such as `menu:<id>`, uses the same path with
  no native input wiring.

#### Event object

- One object is shared by all callbacks on a node for one event:
  `{ type, target, currentTarget, eventId, ...payload }`. `currentTarget` is
  the listener's node. `target` falls back to `currentTarget` when the slot
  is empty or names a destroyed node. `composedPath()` returns an empty array
  once the dispatch ends.
- The mouse target slot is a gpui global in `inca-gpui`. The root container's
  capture listeners clear it for each mouse event. A tree with a mouse
  listener gives every container a marker that runs first among that
  container's mouse listeners. Bubbling runs the deepest container first, and
  the first marker to run keeps the slot. A press also records its own
  container per button, kept until a release of that button has been reported.
  A press of another button leaves it. The window losing activation clears
  every recorded press (`inca_gpui::clear_presses`).
- `eventId` is a number that grows with each event. Every node of one event
  name within one input carries the same value, so an adapter can tell
  whether two calls belong to one event. A dispatch with no input behind it,
  such as the focus events after a focus change, takes a new value per event
  name. One engine has one counter, shared by all its dispatchers, and it
  restarts at 1 with a new engine.
- Base fields: `bubbles`, `cancelable` and `composed` come from one table
  (`EVENTS` in `dispatch.rs`, which also holds each event's target rule,
  movement rule and own-buttons rule; other names get all three `false`). The
  `timeStamp` origin is a process-wide `Instant` fixed when the first
  dispatcher is created. `defaultPrevented` and `eventPhase` come from the
  per-input state the `eventId` lives in. The origin node of that state is the
  first node an event name runs listeners on, in `dispatch` and in
  `EventDispatcher::fire`. A `preventDefault()` call on a cancelable event
  marks the state, so later nodes start with it `true`.
- Methods: `stopPropagation`, `stopImmediatePropagation`, `preventDefault`.
  `preventDefault` reaches the window only for a cancelable event.
- Accessors: `defaultPrevented` (getter), `returnValue` and `cancelBubble`
  (getter and setter with ToBoolean coercion) read the flags the methods set.
  The phase constants and `srcElement` are non-enumerable data properties.
- GPUI's `platform` modifier becomes `metaKey`. `function` is dropped.
  `getModifierState` exists on key and mouse events and reads those four.
  `Accel` reads `platform` on macOS and `control` elsewhere.
- With nothing focused, the window focus rests on a handle the root container
  tracks (`FocusRegistry::parked_handle`, mapped to no node). The root's key
  listeners run and `target` is the root. The root wires `keydown` and
  `keyup` even without a listener. `key` is `"Unidentified"` for an empty
  key, and a letter with Shift held and no `key_char` is upper case.

#### Events

| Name | Payload | Notes |
| --- | --- | --- |
| `click` | mouse + pointer | Fires after `mouseup` at the nearest common ancestor of the press container of that button and the release container, so a press must be recorded. A release over no container uses the root. The payload is `EventPayload::click`: the release position and modifiers, `button` the released button, `buttons` at 0, `detail` the click count, pointer source mouse. A key click fires through `EventSink::key_default` at the focused enabled button with `MousePayload::keyboard_click`. |
| `dblclick` | mouse | Fired through `EventSink::fire` at the `click` ancestor right after the `click` whose release counts 2, with the pointer source cleared. |
| `auxclick` | mouse + pointer | Fired through `EventSink::fire` at the same ancestor for the release of any button but the left one. The payload is `EventPayload::click` with `button` the released button. |
| `contextmenu` | mouse + pointer | The root tracker's capture listener for a right `MouseDownEvent` queues `window.defer`, which runs after the press's bubble. `EventSink::fire` then runs `contextmenu` at the mouse target (the root when none), so it fires whichever way `mousedown` propagated. The payload is `EventPayload::context_menu`: `button` 2, `buttons` 2, `detail` 0, pointer source mouse. |
| `mousedown`, `mouseup` | mouse | `buttons` holds every button currently held. |
| `mousemove` | mouse | `movementX`/`movementY` hold the delta of the raw move. |
| `mouseenter`, `mouseleave` | mouse + `relatedTarget` | Do not bubble. `EventDispatcher::fire` runs only the node's own callbacks, with `target` the node. |
| `mouseover`, `mouseout` | mouse + `relatedTarget` | `EventDispatcher::fire` runs the callbacks of the deepest container, then each ancestor's. |
| `wheel` | mouse + `deltaX`, `deltaY`, `deltaZ`, `deltaMode` | `deltaX`/`deltaY` are GPUI's values negated. `deltaZ` is `0`. `deltaMode` is `0` or `1`. |
| `focus`, `blur` | `relatedTarget` | Do not bubble. |
| `focusin`, `focusout` | `relatedTarget` | `EventDispatcher::fire` runs the node's callbacks, then each ancestor's, with `target` the node. A focus change dispatches `blur`, `focusout`, `focus`, `focusin`. |
| `keydown`, `keyup` | `key`, `repeat`, `location` (0), `isComposing` (false), `ctrlKey`, `shiftKey`, `altKey`, `metaKey`, `getModifierState` | Go to the focused node, or the root when none is focused, and bubble to its ancestors. `keyup` has `repeat: false`. |

Every container of a tree with a mouse listener (`track`, non-inert) wires one
`on_hover`, which calls `EventSink::hover_changed` and keeps the hovered set.
The first container to report in a pointer move calls `EventSink::pointer_over`
with itself as the deepest one, which reports at once. Any other change (a layout change under a
still pointer, a window exit) reports once at the end of the update from the
deepest container of the hovered set. A
report compares the deepest container with the previous one
(`EventDispatcher::hover_prev`) and fires, per `hover_path_difference`:
`mouseout` at the old container, `mouseleave` on each container left
(innermost first), `mouseover` at the new one, `mouseenter` on each container
entered (outermost first). `relatedTarget` of the first two is the new
container and of the last two the old one. A removed old container receives
no `mouseout` or `mouseleave`, and its nearest remaining ancestor is the old
one. Position and modifiers are read from the window when the report runs.

The mouse payload is `clientX`, `clientY`, `screenX`, `screenY`, `x`, `y`,
`pageX`, `pageY`, `movementX`, `movementY`, `button`, `buttons`, `detail`,
`relatedTarget` (`MousePayload::related_target`, `null` on events other than
the hover events) and the four modifier flags. `x`/`y` and `pageX`/`pageY`
equal `clientX`/`clientY`. The sink sets `screen_x`/`screen_y` from
`Window::bounds` plus the client position (`with_screen`).
`MousePayload::pointer` adds `pointerId`, `pointerType`, `isPrimary`, `width`,
`height` and `pressure`: mouse is 1, `"mouse"`, true, 1, 1 and keyboard is -1,
`""`, false, 1, 1. `pressure` is 0.5 when `buttons` is non-zero, else 0.

`click`, `dblclick`, `auxclick` and `contextmenu` keep the `buttons` of
their own payload. The other mouse events take it from the held-button tracker.

`movementX`/`movementY` come from raw pointer moves, which the host records
once per window. Only `mousemove` takes them. Every other event holds 0.

#### Propagation and cancellation

- Callbacks run in the bubble phase only.
- `stopImmediatePropagation()` stops the remaining callbacks on that node.
- `stopPropagation()` is read back after the node's callbacks and forwarded
  to GPUI, which also stops its own ancestor listeners.
- For `wheel`, `stopPropagation()` skips the later `wheel` callbacks. The
  container still scrolls.
- `EventDispatcher::dispatch` and `fire` drop the callbacks of `mousedown`,
  `mouseup`, a mouse `click` and `dblclick` for a disabled button and its
  ancestors. A key click always runs. Listeners below the button run.
- `preventDefault()` reaches only what GPUI honours: the focus change and
  the blur of a `mousedown` and wheel scrolling. For a `button`,
  `preventDefault()` in `keydown` or `keyup` also ends the key click.

#### Ordering

- `focusNode`/`blurNode` queue and apply in order on the next frame.
- The host reads `tabindex` on `setAttribute`/`removeAttribute` (key
  compared case-insensitively), `disabled` on a button, and `hidden`,
  `inert` and `display` changes, which re-check the node's subtree. Moving a
  node re-checks its subtree too.
  The host applies the change before the queued focus requests of that
  frame. `build_spec_with` gives an `inert` node and its subtree an empty
  listener mask. The user rules are in `docs/reference/events.md`.
- A capture-phase `key_down` and `key_up` listener on the root container
  defers until the key's dispatch ended, then calls `EventSink::key_default`.
  It acts when `window.default_prevented()` is unset. `Tab` and `Shift+Tab`
  move focus (Control, Alt and Meta cancel the move). Enter clicks a focused
  enabled button on key down. Space records the button in `space_down` and
  clicks on key up with any modifier. Another key down, a mouse press
  (`EventSink::pointer_pressed`) or a Space key up clears the pending Space.
  `FocusRegistry::tab_move` collects the handled nodes in tree order and
  focuses the target, or blurs for the end of the order. `apply_pending`
  reports the transition on the next frame.
- The root tracker's capture listener for a `MouseUpEvent` queues
  `window.defer`, which runs after every listener of the release and calls
  `EventSink::pointer_released`. The click events fire after that `mouseup`,
  then the press of that button is forgotten.
- Destroying a focused node fires no `blur`.
- All `mousemove` dispatches from one raw event share one movement value.
- The root wires `on_modifiers_changed` into `EventSink::modifiers_changed`.
  The dispatcher diffs the flags against `held_modifiers`, which
  `sync_modifiers` seeds from the window on the first frame and on every
  activation. It bubbles `keydown` or `keyup` named `Shift`, `Control`, `Alt`
  or `Meta`, in that order, from the focused node or the root.
- `HostedApp` observes the window's activation. A change dispatches `blur` and
  `focusout`, or `focus` and `focusin`, to the focused node
  (`FocusTransition::window_activation`). `FocusRegistry::set_window_blurred`
  holds back a later `blur` of that node while the window is inactive. A node
  focused while the window is inactive receives `focus` and `focusin` on
  activation.
- An inert scroll container records its offset in `scroll_recorder` like any
  other, and `settle_wheel` always restores it and leaves the wheel to the
  next container.

## Application menu

The host sets the menu bar (`crates/inca-host/src/menu.rs`). It holds one
menu, titled after the app, with one item, `Quit`, bound to `secondary-q`
(`cmd-q` on macOS, `ctrl-q` elsewhere). `Quit` belongs to the host, so every
app has a way to quit. Only macOS draws a menu bar today.

An item an app defines is dispatched on `rootNodeId()` as the event
`menu:<id>`, through `addEventListener`. An item `save` arrives as
`menu:save`.
