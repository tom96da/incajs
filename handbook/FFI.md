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
| `"button"` | A styled container. The host reads its `tabindex` with a default of 0. [Elements](../docs/reference/elements.md#button) covers its key clicks. |
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
| `addEventListener` | `(nodeId: number, event: string, callbackId: number, capture?: boolean, passive?: boolean) => void` | Register a callback for an event, see [Registering](#registering). `capture` and `passive` default to `false`. |
| `removeEventListener` | `(nodeId: number, event: string, callbackId: number) => boolean` | Drop one registration and return whether it was there. An unknown node returns `false`. |
| `dispatchEvent` | `(nodeId: number, event: Event) => boolean` | Walk a script-built event over the path of `nodeId` and return `false` when a listener canceled it, see [Dispatching from script](#dispatching-from-script). |
| `click` | `(nodeId: number) => void` | Walk an untrusted `click` at `nodeId`, see [Dispatching from script](#dispatching-from-script). |
| `destroyNode` | `(nodeId: number) => number[]` | Free `nodeId` and its subtree, and return every `callbackId` registered in it. An unknown id returns `[]`. Destroying the root throws. Also clears the focus state of the destroyed nodes, and `blur` and `focusout` stay unfired. |
| `focusNode` | `(nodeId: number) => void` | Request focus for `nodeId`. Takes effect next frame. A node with a `tabindex`, and only that node, takes focus. |
| `blurNode` | `(nodeId: number) => void` | Request that `nodeId` lose focus. Takes effect next frame, and does nothing if `nodeId` is not the focused node by then. |

Shared throw rules, all as a `TypeError`:

- A malformed `nodeId` or `callbackId` throws.
- `setAttribute` and `setStyle` throw for a `value` that is not a string, number or boolean.
- `dispatchEvent` throws for an argument that is not an `Event` or
  `CustomEvent` of the host.
- `addEventListener`, `dispatchEvent`, `setAttribute`, `removeAttribute`,
  `setStyle`, `removeStyle` and `appendChild` throw for an unknown node.
  `insertBefore` throws for an unknown parent, child or anchor.
- `removeChild` throws for an unknown `parentId` and ignores an unknown or unattached `childId`.

`dispatchEvent` also throws a `DOMException` named `InvalidStateError` for an
event that is being dispatched.

`insertBefore` and `appendChild`:

- If `anchorId` equals `childId` and the child is already under `parentId`,
  nothing changes.
- If `anchorId` is a real node that is not a child of `parentId`, the child
  is appended at the end.
- An `anchorId` that names no node throws.
- A `childId` naming the root throws.

The host converts the `VirtualNode` tree into GPUI elements on each frame.

### Event dispatch

The host dispatches native input to the JS callbacks registered for a node
and event name. A walk is one dispatch of an event along its path. Payloads
and author-facing behaviour are in [events.md](../docs/reference/events.md).

#### Registering

- Distinct `callbackId`s stack on one `(nodeId, event)`. Re-adding an id is a
  no-op that keeps its first `capture` and `passive`, which belong to the
  registration. An id belongs to one registration, so `removeEventListener`
  takes the id only. `destroyNode` returns the dropped ids because only the
  caller can free the JS half.
- `packages/core` keys a registration by node, event, capture and listener, so
  `removeEventListener(type, listener, options)` finds it by identity and
  adding the same listener again changes nothing. A listener object keeps its
  identity and receives `handleEvent` calls. `once` removes the registration
  before the listener runs. `createEventTarget` builds the listener methods
  for one node, and `window` resolves `rootNodeId()` per call.
- The caller stores the function at `globalThis.__inca_callbacks__[callbackId]`
  before registering, and the host stores the `u32` id. A missing or
  non-function entry is skipped.
- A throwing callback goes to `Host::failures`, and sibling callbacks still
  run. Unhandled promise rejections and failed jobs are reported through the
  job drain before listener errors, and only listener errors go to
  `Host::failures`. Failures are reported in order after each walk, after
  `drain_jobs_and_refresh` and by the development window's load poll
  (`EventDispatcher::report_pending`).

#### Event object

- A walk passes one object to every listener on the path. The host creates
  it for its own events and a script builds it for the events it dispatches.
  Own fields: `type`, `target`, `currentTarget`, `eventId` and the payload.
  A property one listener sets is visible to later listeners.
- `target` is the node the event is fired at, except where Pointer events and
  Keyboard and focus say otherwise. `currentTarget` is the node whose
  listeners run. After the walk it is `null`, `eventPhase` is 0,
  `composedPath()` is empty and the stop flags are clear. `defaultPrevented`
  keeps its value.
- `eventId` grows with each event and is shared by all listeners of a walk.
  Only an event that reaches a listener enters the engine, drains jobs,
  requests a redraw and takes a number. One counter (`Host::last_event_id`)
  serves an engine's dispatchers and restarts at 1 with a new engine.
- `bubbles`, `cancelable` and `composed` come from one table (`EVENTS` in
  `dispatch.rs`) that also holds each event's movement rule and own-buttons
  rule. Other names get every rule `false`. The `timeStamp` origin is a
  process-wide `Instant` fixed when the first dispatcher is created.
- The object inherits from a per-engine prototype holding `stopPropagation`,
  `stopImmediatePropagation`, `preventDefault`, `composedPath`, the phase
  constants (non-enumerable, also on `Event`) and the accessors
  `defaultPrevented`, `returnValue` and `cancelBubble` (settable, ToBoolean)
  and `srcElement` (reads `target`). Members read a host-private `WeakMap`
  record in the engine's userdata (`crates/inca-bridge/src/event.rs`), and a
  call on any other receiver throws a `TypeError`.
- GPUI's `platform` modifier becomes `metaKey` and `function` is dropped.
  `getModifierState` exists on key and mouse events and reads those four.

#### Propagation and cancellation

- The capture pass runs the `capture` callbacks from the root down to the
  target, with `eventPhase` 1 on ancestors and 2 on the target. The bubble
  pass runs the other callbacks from the target up while the event bubbles,
  with `eventPhase` 2 on the target and 3 on ancestors. A non-bubbling event
  runs the capture pass over the whole path and the bubble pass on the target
  only. At the target, capture callbacks run first.
- The path is fixed when the walk starts. Each pass reads a node's listeners
  when it reaches the node, so a listener removed earlier is skipped and one
  added during the node's callbacks skips that pass.
- `stopPropagation()` lets the remaining callbacks of that node in the
  current pass run and gives every later node and pass none. The target's
  capture and bubble callbacks are separate passes, so a stop in the first
  skips the second. `stopImmediatePropagation()` also stops the remaining
  callbacks of that node. Both leave gpui's own propagation running.
- `preventDefault()` in a non-passive callback marks a cancelable event, and
  later listeners read `defaultPrevented` as `true`. `stopPropagation()` works
  in passive callbacks too.

#### Pointer events

- The target of `mousedown`, `mouseup`, `mousemove`, `wheel` and
  `contextmenu` is the deepest container under the pointer, or the root when
  there is none.
- While any listener is registered or any node can take focus, every non-inert
  container reports the pointer (`EventSink::tracks_pointer`) through a marker
  listener and one `on_hover` (`EventSink::hover_changed`), which keeps the
  hovered set. An inert node and its subtree stay outside hit testing and
  hover.
- An inert scroll container records its offset in `scroll_recorder` like any
  other, and `settle_wheel` always restores it and leaves the wheel to the
  next container. A canceled `wheel` rolls the scroll back.
- The mouse target slot is a gpui global in `inca-gpui`. The root's tracker
  clears it in the capture phase of each mouse event, and the first marker to
  run, the deepest container, keeps it. The tracker is the root's first child,
  so its bubble handlers run after every container below. It reports each
  input once (`EventSink::pointer_input`), and the click events fire after the
  `mouseup` input.
- `EventDispatcher::fire` and the pointer walks skip the callbacks of
  `mousedown`, `mouseup`, a mouse `click` and `dblclick` for a disabled
  button and its ancestors. Listeners below the button run, and a key click
  always runs.
- A press record keeps the press container per button until a release of that
  button is reported. A press of another button leaves the record. Window
  deactivation clears every record (`inca_gpui::clear_presses`).
- `click` fires at the nearest common ancestor of the press container of that
  button and the release container, so a press must be recorded. A release
  over no container uses the root. The payload is `EventPayload::click`: the
  release position and modifiers, `button` the released button, `buttons` 0,
  `detail` the click count of the press, pointer source mouse.
- `dblclick`, `auxclick` and `contextmenu` fire through `EventSink::fire`.
  `dblclick` fires at the `click` ancestor right after the `click` whose press
  counts 2, with the pointer source cleared. `auxclick` fires there for the
  release of any non-left button, with the `click` payload. The root tracker
  fires `contextmenu` at the target of a right `MouseDownEvent` right after
  the `mousedown` input, with `EventPayload::context_menu`: `button` 2,
  `buttons` 2, `detail` 0, pointer source mouse.
- The first container to report in a pointer move calls
  `EventSink::pointer_over` with itself as the deepest and reports at once.
  Any other change reports once at the end of the update from the deepest
  container of the hovered set. Each report compares it with the previous
  deepest (`EventDispatcher::hover_prev`, `hover_path_difference`) and fires
  `mouseout` at the old one, `mouseleave` on each container left (innermost
  first), `mouseover` at the new one, `mouseenter` on each container entered
  (outermost first). The `mouseover`/`mouseout` walk starts at the deepest
  container. `relatedTarget` is the new container for `mouseout` and
  `mouseleave` and the old one for `mouseover` and `mouseenter`. A removed old
  container is skipped and its nearest remaining ancestor counts as the old
  one. Position and modifiers are read at the report.

#### Mouse payload

- The payload is `clientX`, `clientY`, `screenX`, `screenY`, `x`, `y`,
  `pageX`, `pageY`, `movementX`, `movementY`, `button`, `buttons`, `detail`,
  `relatedTarget` (`null` except on hover events) and the four modifier flags.
  `x`/`y` and `pageX`/`pageY` equal `clientX`/`clientY`, and the sink sets
  `screenX`/`screenY` from `Window::bounds` plus the client position.
- `MousePayload::pointer` adds `pointerId`, `pointerType`, `isPrimary`,
  `width`, `height` and `pressure`: mouse is 1, `"mouse"`, true, 1, 1 and
  keyboard is -1, `""`, false, 1, 1. `pressure` is 0.5 when `buttons` is
  non-zero, else 0.
- `click`, `dblclick`, `auxclick` and `contextmenu` keep the `buttons` of
  their own payload. Other mouse events take it from the held-button tracker,
  which every raw `mousedown` and `mouseup` updates.
- `movementX`/`movementY` come from raw pointer moves, recorded once per
  window. Only `mousemove` takes them, and every other event holds 0.

#### Keyboard and focus

- The target of `keydown` and `keyup` is the focused node, or the root when
  nothing is focused. The window focus then rests on
  `FocusRegistry::parked_handle`, which maps to no node.
- The root wires capture-phase `keydown` and `keyup` listeners whether or not
  a script listens. They call `EventSink::key_input`, which walks the event
  and then runs the default action of an event left uncanceled. `key` is
  `"Unidentified"` for an empty key, and a letter with Shift held is upper
  case when `key_char` is absent.
- `Tab` and `Shift+Tab` move focus, and Control, Alt and Meta cancel the move.
  `FocusRegistry::tab_move` collects the handled nodes in tree order and
  focuses the target, or blurs at the end of the order.
- Enter clicks a focused enabled button on key down. Space clicks it on key
  up with any modifier. Another key down, a mouse press
  (`EventSink::pointer_pressed`) or a Space key up clears the pending Space.
  A key click uses `MousePayload::keyboard_click`, and `preventDefault()` in
  `keydown` or `keyup` of the `button` ends it.
- A canceled `keydown` or `keyup` calls `Window::prevent_default` and
  `App::stop_propagation` after its walk. The platform then treats the key as
  handled, so later gpui key listeners stay idle. A stopped or unlistened key
  stays unhandled, so gpui key listeners still run. Keybindings run before key
  listeners, so a bound action consumes its key before any walk.
- The root wires `on_modifiers_changed` into `EventSink::modifiers_changed`.
  A modifier change bubbles `keydown` or `keyup` named `Shift`, `Control`,
  `Alt` or `Meta`, in that order, from the focused node or the root. It is
  diffed against `held_modifiers`, which `sync_modifiers` seeds from the
  window on the first frame and on every activation.
- `focusNode`/`blurNode` queue and apply in order on the next frame. A focus
  change dispatches `blur`, `focusout`, `focus`, `focusin` in that order, and
  the `focusin`/`focusout` walk has `target` the node. Changes to `tabindex`
  (key compared case-insensitively), `disabled` on a button, `hidden`,
  `inert` and `display`, and moving a node, re-check the node's subtree. This
  applies before that frame's queued focus requests.
- An uncanceled `mousedown` focuses the nearest node on the target's path
  that has a `FocusRegistry` handle, and blurs the window when there is none.
  The tracker's capture handler calls `Window::prevent_default` to stop gpui's
  focus-on-press.
- A window activation change dispatches `blur` and `focusout`, or `focus` and
  `focusin`, to the focused node. While the window is inactive a later `blur`
  of that node is held back, and a node focused meanwhile receives `focus` and
  `focusin` on activation.

#### Dispatching from script

- `Event` and `CustomEvent` (`event.rs`) share the prototype and `WeakMap`
  record, with `isTrusted` false and `target` `null`. `Event(type, init)` and
  `CustomEvent(type, init)` need `new`, a type, and an `init` that is an
  object, `null` or `undefined`. Any other call throws a `TypeError`.
  `CustomEvent.prototype` inherits from `Event.prototype`.
- `dispatchEvent` runs the walk in the caller's context, so a listener can
  call it. It sets `target` and `isTrusted`, reads `type` and the flags
  (`bubbles`, `cancelable`, `composed`) from the event's record and returns
  the negated canceled flag. Default actions and the job queue belong to the
  caller. The `dispatching` flag is set last and cleared before the event
  object is touched. A listener that freezes the event therefore clears the
  flag, and dispatching that frozen event again throws a `TypeError` when it
  sets `target`.
- `click()` walks a `click` event with `MousePayload::keyboard_click`.
  `Host::clicking` holds the nodes whose click is running, as a re-entrancy
  guard. A disabled `button` and an unknown node return at once.

#### Event table

| Name | Payload |
| --- | --- |
| `click`, `auxclick`, `contextmenu` | mouse + pointer |
| `dblclick`, `mousedown`, `mouseup`, `mousemove` | mouse |
| `mouseenter`, `mouseleave`, `mouseover`, `mouseout` | mouse + `relatedTarget` |
| `wheel` | mouse + `deltaX`, `deltaY`, `deltaZ`, `deltaMode` |
| `focus`, `blur`, `focusin`, `focusout` | `relatedTarget` |
| `keydown`, `keyup` | `key`, `repeat`, modifier flags, `getModifierState` |

`wheel` negates GPUI's `deltaX` and `deltaY`, `deltaZ` is 0 and `deltaMode` is
0 or 1.

## Application menu

The host sets the menu bar (`crates/inca-host/src/menu.rs`). It holds one
menu, titled after the app, with one item, `Quit`, bound to `secondary-q`
(`cmd-q` on macOS, `ctrl-q` elsewhere). `Quit` belongs to the host, so every
app has a way to quit. Only macOS draws a menu bar today.

An item an app defines is dispatched on `rootNodeId()` as the event
`menu:<id>`, through `addEventListener` and the same registry. An item
`save` arrives as `menu:save`.
