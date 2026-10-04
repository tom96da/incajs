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

Implemented in `crates/inca-gpui/src/element.rs`. There are two kinds:

| `tag_name` | Maps to |
| --- | --- |
| `"text"` | Content is its `"value"` string attribute (missing/non-string → empty, never a panic) followed by its descendants' text in child order. Descendants are not rendered as separate elements. |
| anything else | A generic styled container (a GPUI `div()`). |

### Style prop vocabulary

Implemented in `element.rs`. Unrecognized keys and malformed enum strings are
ignored without an error. The table lists every key the host reads.

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
| `createNode` | `(tag: string) => number` | Allocate a `VirtualNode`, return its id. |
| `appendChild` | `(parentId: number, childId: number) => void` | Attach a child at the end of `parentId`'s children. Same as `insertBefore` with no anchor. |
| `insertBefore` | `(parentId: number, childId: number, anchorId: number \| null) => void` | Attach a child before `anchorId`, or at the end if `null`. |
| `removeChild` | `(parentId: number, childId: number) => void` | Detach a child node. |
| `setAttribute` | `(nodeId: number, key: string, value: string \| number \| boolean) => void` | Set a non-style attribute prop. |
| `setStyle` | `(nodeId: number, key: string, value: string \| number \| boolean) => void` | Set a style prop. |
| `removeStyle` | `(nodeId: number, key: string) => void` | Remove a style prop so it renders as if never set. Removing a key that isn't set does nothing. |
| `addEventListener` | `(nodeId: number, event: string, callbackId: number) => void` | Register a callback for an event. Ids stack per `(nodeId, event)`, see [Registering](#registering). |
| `removeEventListener` | `(nodeId: number, event: string, callbackId: number) => boolean` | Drop one registration and return whether it was there. An unknown node returns `false`. |
| `destroyNode` | `(nodeId: number) => number[]` | Free `nodeId` and its subtree, and return every `callbackId` registered in it. An unknown id returns `[]`. Destroying the root throws. Also clears the focus state of the destroyed nodes. |
| `focusNode` | `(nodeId: number) => void` | Request focus for `nodeId`. Takes effect next frame, on any node. An unknown or destroyed node is ignored. |
| `blurNode` | `(nodeId: number) => void` | Request that `nodeId` lose focus. Takes effect next frame, and does nothing if `nodeId` is not the focused node by then. |

Shared throw rules, all as a `TypeError`:

- A malformed `nodeId` or `callbackId` throws.
- `setAttribute` and `setStyle` throw for a `value` that is not a string, number or boolean.
- `addEventListener`, `setAttribute`, `setStyle`, `removeStyle` and `appendChild` throw for an unknown node. `insertBefore` throws for an unknown parent, child or anchor.
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
  `{ type, target, currentTarget, ...payload }`. `target` equals
  `currentTarget`.
- Methods: `stopPropagation`, `stopImmediatePropagation`, `preventDefault`.
- GPUI's `platform` modifier becomes `metaKey`. `function` is dropped.

#### Events

| Name | Payload | Notes |
| --- | --- | --- |
| `click` | none | Fires before `mouseup` on the same node. |
| `mousedown`, `mouseup` | mouse | `buttons` holds every button currently held. |
| `mousemove` | mouse | |
| `mouseenter`, `mouseleave` | mouse | Do not bubble. Position and modifiers are read when hover changes. |
| `wheel` | mouse + `deltaX`, `deltaY`, `deltaZ`, `deltaMode` | `deltaX`/`deltaY` are GPUI's values negated. `deltaZ` is `0`. `deltaMode` is `0` or `1`. |
| `focus`, `blur` | none | Do not bubble. |
| `keydown`, `keyup` | `key`, `repeat`, `ctrlKey`, `shiftKey`, `altKey`, `metaKey` | Go to the focused node and bubble to its ancestors. `keyup` has `repeat: false`. |

The mouse payload is `clientX`, `clientY`, `pageX`, `pageY`, `movementX`,
`movementY`, `button`, `buttons`, `detail` and the four modifier flags.
`pageX`/`pageY` equal `clientX`/`clientY`.

`movementX`/`movementY` come from raw pointer moves, which the host records
once per window.

#### Propagation and cancellation

- Callbacks run in the bubble phase only.
- `stopImmediatePropagation()` stops the remaining callbacks on that node.
- `stopPropagation()` is read back after the node's callbacks and forwarded
  to GPUI, which also stops its own ancestor listeners.
- For `wheel`, `stopPropagation()` skips the later `wheel` callbacks. The
  container still scrolls.
- `preventDefault()` reaches only what GPUI honours: focus on `mousedown`,
  the `click` from `Enter`/`Space`, and wheel scrolling.

#### Ordering

- `focusNode`/`blurNode` queue and apply in order on the next frame.
- Destroying a focused node fires no `blur`.
- All dispatches from one raw event share one movement value.

## Application menu

The host sets the menu bar (`crates/inca-host/src/menu.rs`). It holds one
menu, titled after the app, with one item, `Quit`, bound to `secondary-q`
(`cmd-q` on macOS, `ctrl-q` elsewhere). `Quit` belongs to the host, so every
app has a way to quit. Only macOS draws a menu bar today.

An item an app defines is dispatched on `rootNodeId()` as the event
`menu:<id>`, through `addEventListener`. An item `save` arrives as
`menu:save`.
