<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Host bridge (FFI) reference

The function surface exposed to JS as `globalThis.__inca_native__`, bound
into the QuickJS context by the Rust host via `rquickjs`. JS and Rust run in
one process, and a call is an in-process function call with typed arguments.
No C ABI or IPC is involved.

## JS values stay in JS

Rust keeps ids and plain data. A handle kept past the call that produced it
outlives the context it belongs to, so every function and object lives on the
JS side. [Registering](#registering) is the implementation example: listener
functions stay in JS and Rust holds only the callback ids. Values the host
keeps for itself, such as the promise of a pending module and the event store,
sit in a per-realm vault (`inca_jsenv::vault`), a null-prototype object. A
read-only global function returns it only while Rust calls it through a native
gate, so no script reaches it, and Rust holds only the keys. The data of a
native class instance is the exception. An event object is an instance of a
native class. The JS values its fields hold, such as `target` and `detail`,
belong to that instance. The garbage collector traces them, and they go when
the event goes. They sit in one struct whose trace names every field. A value
left untraced makes the engine's drop abort the process.

All `unsafe` code of the Rust crates lives in `inca-jsenv/src/quickjs.rs`.
The one exception is the `unsafe impl` that rquickjs's `JsLifetime` derive
expands to, which gives a Rust value on a JS object its lifetime marker.
`clippy.toml` restricts that derive to the files that allow it:
`inca-jsenv/src/sourcemap/store.rs` and `inca-bridge/src/event/record.rs`. It
restricts `rquickjs::Persistent` too, since one kept past its runtime aborts
the process when the runtime drops.

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
- Detaching, moving or destroying a node drops the focus its subtree holds
  at the call, with no `blur` or `focusout`.
- Making a node its own ancestor throws. A `childId` naming the root, which
  belongs to the host, throws too.
- A `nodeId` that is not a finite whole number in the `u32` range throws a
  `TypeError`.
- `destroyNode` frees a node and its subtree at once. At the start of each
  frame the host also frees every node that has no parent and is neither
  the root nor held with `holdNode`, with its subtree.

### Tag vocabulary

Implemented in `crates/inca-gpui/src/element/`. There are four kinds:

| `tag_name` | Maps to |
| --- | --- |
| `"text"` | Content is its `"value"` string attribute (missing/non-string → empty, never a panic) followed by its descendants' text in child order. Descendants are not rendered as separate elements. |
| `"button"` | A styled container. The host reads its `tabindex` with a default of 0. [Elements](../docs/reference/elements.md#button) covers its key clicks. |
| `"input"` | One line of editable text in a styled container. The host keeps its text; `inputValue` and `setInputValue` read and write it. Its children are not rendered. [Elements](../docs/reference/elements.md#input) covers the rest. |
| anything else | A generic styled container (a GPUI `div()`). |

### Style prop vocabulary

Implemented in `element/parse.rs` and `element/style.rs`.
`inca_gpui::STYLE_KEYS` lists every key, its CSS names and the value it takes.
An unrecognized key, an invalid color and a value of a shape the host does not
support each print one warning line on stderr, once for each node and warning
line, and the value reads as unset. An empty string `''` on a known key
prints no warning. The table lists every key the host reads. `set_style` and
`remove_style` map the camelCase and kebab-case spellings of a key to its
snake_case form, and a CSS name to its host key, as `inca_gpui::STYLE_KEYS`
lists. All spellings of one prop share one slot. A key with an uppercase initial, a mixed-case kebab key and a key
that starts with `-` are unknown.

A px value is a number or a string: a decimal with optional sign and exponent,
then `px` in any case (`"10px"`), or a string that reads as zero (`"0"`).
Surrounding whitespace is ignored. A decimal value is a number or a string
holding one decimal (`"0.5"`). Neither takes `%`. Each key keeps its range;
`opacity` clamps, any other value outside its range warns.

| Key | Value | Maps to |
| --- | --- | --- |
| `display` | `"flex"`\|`"block"`\|`"grid"`\|`"none"` | `Style::display` |
| `flex_direction` | `"row"`\|`"column"`\|`"row-reverse"`\|`"column-reverse"`\|`"row_reverse"`\|`"column_reverse"` | `flex_direction` |
| `justify_content` / `align_items` | `"start"`\|`"end"`\|`"flex-start"`\|`"flex-end"`\|`"center"`\|`"stretch"` | resp. fields |
| `gap` | px value (`>= 0`) | `gap.width` & `gap.height` (uniform) |
| `width` / `height` | px value (`>= 0`) or `"auto"` | `size.width` / `size.height` |
| `border_width` | px value (`>= 0`) | all four `border_widths.*` (uniform) |
| `background` / `border_color` / `text_color` | number from `0` to `0xffffff`, or string (`"#rgb"`, `"#rgba"`, `"#rrggbb"`, `"#rrggbbaa"`, `"transparent"`, `rgb()`, `rgba()`); any other value except `''` warns | `Fill`/`Hsla` |
| `corner_radius` | px value (`>= 0`) | all four `corner_radii.*` (uniform) |
| `text_size` | px value (`>= 0`) | `text.font_size` |
| `overflow` / `overflow_x` / `overflow_y` | `"visible"`\|`"hidden"`\|`"clip"`\|`"scroll"`\|`"auto"` (= `"scroll"`); `overflow` also takes `"<x> <y>"` | `overflow.x` / `overflow.y`; an axis key beats `overflow`; beside a `hidden` or `scroll` axis an unset or `visible` axis becomes `scroll` and `clip` becomes `hidden`; `hidden` clips and takes script scrolling, `clip` clips without scrolling, only `scroll` takes the wheel |
| `padding` (+ `_x`/`_y`/`_top`/`_right`/`_bottom`/`_left`) | px value (`>= 0`) | `padding.*`; side beats axis beats shorthand, a wrong-typed value falls through |
| `margin` (+ the same suffixes) | px value (negatives allowed) or `"auto"` | `margin.*`, same precedence |
| `flex_grow` / `flex_shrink` | decimal value `>= 0` | `flex_grow` / `flex_shrink` |
| `opacity` | decimal value, clamped to `0..=1` (`NaN` ignored) | `opacity` |
| `min_width` / `min_height` / `max_width` / `max_height` | px value (`>= 0`) or `"auto"` | `min_size.*` / `max_size.*` |

A color string ignores surrounding ASCII whitespace. `rgb()` and `rgba()` take
three channels and an optional alpha, in the comma syntax (all numbers or all
percentages) or the space syntax (`rgb(1 2 3 / 0.5)`, numbers and percentages
may mix). A channel and the alpha saturate at their limits and round half up.
The alpha has 256 steps.

`text_color` and `text_size` apply to containers. A text leaf takes them from
the nearest ancestor container.

A scroll container (`overflow` `hidden` or `scroll` on an axis) carries a
zero-size, absolutely placed child before its children, after the root's
pointer tracker.

## Binding functions

The functions live on `globalThis.__inca_native__`, a read-only,
non-configurable global. A parameter marked `?` may be left out, and
`undefined` or `null` there means its default. A parameter typed `| null` must
be passed, and `undefined` there counts as `null`.

| Function | Signature | Purpose |
| --- | --- | --- |
| `rootNodeId` | `() => number` | Return the id of the host-allocated root container, the node a mounting app attaches itself under. |
| `createNode` | `(tag: string) => number` | Allocate a `VirtualNode`, return its id. Throws once every id has been used. Freed at the next frame unless inserted or held, see Tree rules. |
| `appendChild` | `(parentId: number, childId: number) => void` | Attach a child at the end of `parentId`'s children. Same as `insertBefore` with no anchor. |
| `insertBefore` | `(parentId: number, childId: number, anchorId: number \| null) => void` | Attach a child before `anchorId`, or at the end if `null`. |
| `removeChild` | `(parentId: number, childId: number) => void` | Detach a child node. Freed at the next frame unless re-inserted or held, see Tree rules. |
| `holdNode` | `(nodeId: number) => void` | Keep `nodeId` and its subtree alive while it has no parent. |
| `releaseNode` | `(nodeId: number) => void` | End the hold. |
| `setAttribute` | `(nodeId: number, key: string, value: string \| number \| boolean) => void` | Set a non-style attribute prop. |
| `getAttribute` | `(nodeId: number, key: string) => string \| number \| boolean \| null` | Read a non-style attribute prop as stored, or `null` when the key is unset. |
| `removeAttribute` | `(nodeId: number, key: string) => void` | Remove a non-style attribute prop. Removing a key that isn't set does nothing. |
| `setStyle` | `(nodeId: number, key: string, value: string \| number \| boolean) => void` | Set a style prop. |
| `removeStyle` | `(nodeId: number, key: string) => void` | Remove a style prop so it renders as if never set. Removing a key that isn't set does nothing. |
| `inputValue` | `(nodeId: number) => string` | The text of the input node `nodeId`. Throws a `TypeError` for a node of another tag. |
| `setInputValue` | `(nodeId: number, value: string) => void` | Replace the text of the input node `nodeId`, without CR and LF. A changed text puts the caret at its end and ends a composition. Throws a `TypeError` for a node of another tag. |
| `addEventListener` | `(target: number \| "$document" \| "$window", event: string, capture?: boolean, passive?: boolean) => number` | Register a callback for an event and return its id, see [Registering](#registering) and [Tagged targets](#tagged-targets). `capture` and `passive` default to `false`. |
| `removeEventListener` | `(target: number \| "$document" \| "$window", event: string, callbackId: number) => boolean` | Drop one registration and return whether it was there. An unknown node returns `false`. |
| `dispatchEvent` | `(target: number \| "$document" \| "$window", event: Event) => boolean` | Walk a script-built event over the path of `target` and return `false` when a listener canceled it, see [Dispatching from script](#dispatching-from-script) and [Tagged targets](#tagged-targets). |
| `click` | `(nodeId: number) => void` | Walk an untrusted `click` at `nodeId`, see [Dispatching from script](#dispatching-from-script). |
| `setPointerCapture` | `(nodeId: number, pointerId: number) => void` | Make `nodeId` the pending pointer capture while a button is held, see [Pointer events](#pointer-events). Throws `NotFoundError` for a `pointerId` other than 1 and `InvalidStateError` for a node outside the mounted tree or that names no node. |
| `releasePointerCapture` | `(nodeId: number, pointerId: number) => void` | Clear the pending capture when `nodeId` holds it. Throws `NotFoundError` for a `pointerId` other than 1. |
| `hasPointerCapture` | `(nodeId: number, pointerId: number) => boolean` | Whether `nodeId` is the pending capture and still mounted. `false` for a `pointerId` other than 1. |
| `destroyNode` | `(nodeId: number) => number[]` | Free `nodeId` and its subtree, and return every `callbackId` registered in it. An unknown id returns `[]`. Destroying the root throws. Also drops the hold and the waiting `scrollIntoViewNode` calls of the destroyed nodes. |
| `focusNode` | `(nodeId: number, preventScroll?: boolean) => void` | Request focus for `nodeId`, next frame. Only a connected node with a `tabindex` that is not hidden takes it, and a `button` or `input` defaults to `0`. Unless `preventScroll` is `true`, the scroll containers above the node then scroll it into view: a node that shows stays put, a node outside a container's box centres, and a node that shows partly moves the least vertically and stays put horizontally. A node with no box yet is retried next frame. |
| `focusedNode` | `() => number \| null` | The id of the focused node, or `null` when none has focus. |
| `blurNode` | `(nodeId: number) => void` | Request that `nodeId` lose focus. Takes effect next frame, and does nothing if `nodeId` is not the focused node by then. |
| `layoutOf` | `(nodeId: number) => [x, y, width, height, border, scrollX, scrollY] \| null` | Read the border box of `nodeId` in window pixels as the last drawn frame placed it, with `border` the uniform border width. `scrollX` and `scrollY` are the positive scroll offset the parent applied to that box. `null` for a node with no recorded box. |
| `scrollOf` | `(nodeId: number) => [left, top, maxLeft, maxTop] \| null` | Read the scroll offset (positive) and its maximum as of the last drawn frame, with the offset clamped to that maximum. `null` for a node whose `overflow` is `visible` or `clip` on both axes. |
| `scrollIntoViewNode` | `(nodeId: number, block: string, inline: string) => void` | Scroll every scroll container above `nodeId` until the node shows, innermost first. `block` is the vertical and `inline` the horizontal alignment: `"start"`, `"center"`, `"end"` or `"nearest"`. `"nearest"` keeps a node that shows in place. A node shorter than the box aligns the edge that lies outside it, and a longer node aligns its end when its start lies before the box, and its start when its end lies after the box. Uses the boxes of the last drawn frame. A node with no recorded box is retried once on the next frame. |
| `scrollNode` | `(nodeId: number, left: number \| null, top: number \| null) => void` | Set the scroll offset of the axes given, a `null` axis keeps its value. Values below 0 become 0, and the host clamps the upper end at the next frame. A node whose `overflow` is `visible` or `clip` on both axes ignores the call. |
| `startTimer` | `(delay: number, repeat: boolean) => number \| undefined` | Queue a timer due `delay` ms from now and return its id, see [Timers](#timers). Ids start at 1, are shared by every timer and are never reused. `undefined` after the last id, `2^31 - 1`. |
| `stopTimer` | `(id: number) => void` | Drop timer `id`. An id that is not queued does nothing. |

Shared throw rules, all as a `TypeError` except where noted:

- A malformed `nodeId`, `callbackId`, timer `id` or `delay` throws, and so
  does a `target` string other than the two tags in
  [Tagged targets](#tagged-targets).
- A `capture`, `passive` or `preventScroll` that is not a boolean,
  `undefined` or `null` throws.
- A `repeat` that is not a boolean throws.
- `addEventListener` throws a `RangeError` after the last id.
- `setAttribute` and `setStyle` throw for a `value` that is not a string, number or boolean.
- `dispatchEvent` throws for an argument that is not an event of the host.
- A `nodeId`, `parentId`, `childId` or `anchorId` that names no node
  throws, except in the calls that let go of something: `releaseNode`,
  `destroyNode`, `removeEventListener`, `blurNode` and the `childId` of
  `removeChild` ignore it. The host can free a node before the caller lets
  go of it.
- `scrollIntoViewNode` throws for a `block` or `inline` outside the four
  alignment names.
- `removeChild` ignores a `childId` that is not a child of `parentId`.

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

The host dispatches native input to the JS callbacks registered for an event
target and event name. A walk is one dispatch of an event along its path.
Payloads and author-facing behaviour are in
[events.md](../docs/reference/events.md).

#### Registering

- `addEventListener` allocates the id and returns it. Each id is issued once.
  The ids run from 0 to `u32::MAX - 1`, and the call after the last one
  throws a `RangeError`. Distinct ids stack on one `(target, event)`, and
  `capture` and `passive` belong to the registration. An id belongs to one
  registration, so `removeEventListener` takes the id only. `destroyNode`
  returns the dropped ids because only the caller can free the JS half.
- `packages/core` keys a registration by target, event, capture and listener, so
  `removeEventListener(type, listener, options)` finds it by identity and
  adding the same listener again changes nothing. A listener object keeps its
  identity and receives `handleEvent` calls. A listener function runs with the
  event's `currentTarget` as `this`. `once` removes the registration before
  the listener runs. `createEventTarget` builds the listener methods for one
  target, a node or a tag.
- `install` creates `globalThis.__inca_callbacks__` as an empty
  null-prototype object on a read-only, non-configurable global. The caller
  stores the function at `globalThis.__inca_callbacks__[id]` after
  `addEventListener` returns `id`. A listener runs only after the call
  returns. A missing or non-function entry is skipped.
- A throwing callback goes to `Host::failures`, and sibling callbacks still
  run. Unhandled promise rejections and failed jobs are reported through the
  job drain before listener errors, and only listener errors go to
  `Host::failures`. Failures are reported in order after each walk, after
  `drain_jobs_and_refresh` and by the development window's load poll
  (`EventDispatcher::report_pending`).

#### Released nodes

`__inca_released__(nodeIds: number[], callbackIds: number[]) => void` is a
global that core defines read-only on each load, so the latest copy of core
answers. Rust looks it up by name on each call and keeps nothing. A missing
global is skipped.

- Rust calls it at most once per frame, after the focus pass, and only when
  the sweep of that frame freed a node.
- `nodeIds` holds every freed node, subtrees included, and `callbackIds`
  every registration on them. Each id arrives once. Ids freed by
  `destroyNode` never pass through it.
- A throw is reported to `Host::failures` and the frame goes on.

#### Event targets

`__inca_targets__(ids: (number | "$document" | "$window")[]) => unknown[]` is a
global that core defines read-only on each load, like `__inca_released__`, so
the latest copy of core answers. Rust looks it up by name on each walk and
keeps nothing.

- A walk that reaches a listener calls it once with the path, target first.
  Entry `i` of the result stands for `ids[i]` in `target`,
  `currentTarget` and `composedPath()`.
- An event whose payload names a related node calls it once more with that
  node, for `relatedTarget`.
- `dispatchEvent` calls it whether or not a listener hears the event.
- When the global is not a function, these fields carry the ids.
- A throw, or a result that is not an array, goes to `Host::failures` and the
  walk runs no listener. `dispatchEvent` throws it.
- Core gives the element handle of a node that has one, the `document` export
  for the `"$document"` tag, the `window` export for the `"$window"` tag, the
  element of the host root, which it creates on first use and `mount()`
  reuses, and `null` for any other node.

#### Tagged targets

The strings `"$document"` and `"$window"` name the `document` and
`window` exports in place of a node id in `addEventListener`, `removeEventListener`,
`dispatchEvent` and `__inca_targets__`. Rust holds them as `EventTargetId`
with the node ids.

- Any other string throws a `TypeError`.
- A registration on a tag takes `capture` and `passive` like one on a node,
  and `document` and `window` keep separate registrations of one event name.
- A path runs from the target node through its ancestors, the host root
  included, to `"$document"`, then `"$window"`. A node outside the root
  reaches neither global. A script dispatch at `"$document"` reaches
  `"$window"`, and one at `"$window"` stays there.
- A cut node silences both tags for that walk.
- Tags never reach `destroyNode`, the sweep or `__inca_released__`. The
  registrations live in the host and end with it, and the callbacks of the
  tags end with the JS realm.

#### Event object

- A walk passes one object to every listener on the path. The host creates
  it for its own events and a script builds it for the events it dispatches.
  Its fields are getters on the prototype of its kind, and its one own
  property is the getter `isTrusted`, shared by every event.
- `target` is the target the event is fired at, except where Pointer events and
  Keyboard and focus say otherwise. `currentTarget` is the target whose
  listeners run. `target`, `currentTarget`, the entries of `composedPath()`
  and `relatedTarget` carry what [Event targets](#event-targets) gives for
  those nodes. After the walk `currentTarget` is `null`, `eventPhase` is 0,
  `composedPath()` is empty and the stop flags are clear. `defaultPrevented`
  keeps its value.
- `eventId` grows with each event and is shared by all listeners of a walk.
  Only an event that reaches a listener enters the engine, drains jobs,
  requests a redraw and takes a number. One counter (`Host::last_event_id`)
  serves an engine's dispatchers and restarts at 1 with a new engine.
- `bubbles`, `cancelable` and `composed` come from one table (`EVENTS` in the
  `event_sink` module of `inca-gpui`) that also holds each event's movement rule and
  own-buttons rule. Other names get every rule `false`. The `timeStamp` origin is a
  process-wide `Instant` fixed when the first dispatcher is created.
- The object is an instance of the native class `IncaEvent`
  (`crates/inca-bridge/src/event/record.rs`), and its fields live in Rust.
  Events of every kind inherit from `Event.prototype`, which holds
  `stopPropagation`, `stopImmediatePropagation`, `preventDefault`,
  `composedPath`, `initEvent`, the phase constants (also on `Event`), the
  getters of the base fields, `returnValue` and `cancelBubble` (settable,
  ToBoolean), `srcElement` (reads `target`) and the non-enumerable `eventId`.
  A getter or a method called on any other receiver throws a `TypeError`. The
  prototypes sit in the realm's vault.
- Each kind of event inherits from a prototype of its kind that holds the
  getters of its payload fields. A UI prototype holds `view`, `detail` and
  `which`, and the focus, mouse and keyboard prototypes inherit from it.
  The pointer events, `click`, `auxclick` and `contextmenu` inherit from a
  pointer prototype and `wheel` from a wheel prototype, both children of the
  mouse one. `beforeinput` and `input` inherit from an input prototype and the
  composition events from a composition prototype, both children of the UI
  one. The mouse and keyboard prototypes hold their own `getModifierState`.
  A field getter or `getModifierState` throws a `TypeError` on an event of
  another kind.
  Scroll events and events built with `new Event` inherit from
  `Event.prototype`, events built with `new CustomEvent` from
  `CustomEvent.prototype`, which holds the `detail` getter, and events built
  with another constructor from the prototype of that class.
  - The walk turns the gpui payload into the payload of the event once, when
    it creates the event (`Payload::from_host`). `which` is `button + 1` on
    `mousedown`, `mouseup`, `click`, `dblclick`, `auxclick` and
    `contextmenu`, the `button` + 1 on every pointer event (0 for a `button`
    of -1), the lowest held button index + 1 on the other mouse events and 0
    with none held, 0 on `wheel`, `keyCode` on key events and 0 on
    focus events. `view` is `null` and `detail` is 0 on key and focus events.
    `keyCode` comes from `key` on a US layout (`event/key_code.rs`).
- GPUI's `platform` modifier becomes `metaKey` and `function` is dropped.
  `getModifierState` converts its argument to a string and reads those four
  fields.

#### Propagation and cancellation

- The capture pass runs the `capture` callbacks from `window` down to the
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
- A stop requested before a walk, through `stopPropagation()`,
  `stopImmediatePropagation()` or `cancelBubble = true`, gives that walk no
  callback. A dispatch that no listener hears clears it too.
- `preventDefault()` in a non-passive callback marks a cancelable event, and
  later listeners read `defaultPrevented` as `true`. `stopPropagation()` works
  in passive callbacks too.

#### Pointer events

- The target of `mousedown`, `mouseup`, `mousemove`, `wheel` and
  `contextmenu` is the deepest container under the pointer, or the root when
  there is none. While a pointer capture is active, the pointer events and
  `mousedown`, `mouseup`, `mousemove` go to the capturing node.
- While any listener is registered or any node can take focus, every non-inert
  container reports the pointer (`EventSink::tracks_pointer`) through a marker
  listener and one `on_hover` (`EventSink::hover_changed`), which keeps the
  hovered set. An inert node and its subtree stay outside hit testing and
  hover.
- The host builds a container with a scroll handle as `hidden`, so `gpui`
  never steps it, and applies one step itself after the `wheel` listeners ran.
  The step goes to the first container under the pointer that is live
  (connected, displayed, not `hidden`, not inert, `overflow` `scroll` on the
  axis) and can still move. It clamps at the end and the remainder goes
  nowhere. The root takes the step only when no container below it could move
  before the listeners. A container that scrolls on both axes takes the larger
  delta, and a tie moves along y.
- A canceled `wheel` applies no step and leaves offsets as the listeners left
  them. A script's write stays, and an uncanceled step adds to it.
- The mouse target slot is a gpui global in `inca-gpui`. The root's tracker
  clears it in the capture phase of each mouse event, and the first marker to
  run, the deepest container, keeps it. The tracker is the root's first child,
  so its bubble handlers run after every container below. It reports each
  input once (`EventSink::pointer_input`), and the click events fire after the
  `mouseup` input.
- `EventDispatcher::fire` and the pointer walks skip the callbacks of
  `mousedown`, `mouseup`, a mouse `click` and `dblclick` for a disabled
  button or input and its ancestors. Listeners below it run, and a key click
  always runs.
- A press record keeps the press container per button until a release of that
  button is reported. A press of another button leaves the record. Window
  deactivation clears every record (`inca_gpui::clear_presses`).
- `click` fires at the nearest common ancestor of the press container of that
  button and the release container, so a press must be recorded. A release
  over no container uses the root. The payload is `EventPayload::click`: the
  release position and modifiers, `button` the released button, `buttons` 0,
  `detail` the click count of the press, pointer source mouse. On macOS a left
  release with Control held completes no `click`.
- `dblclick`, `auxclick` and `contextmenu` fire through `EventSink::fire`.
  `dblclick` fires at the `click` ancestor right after the `click` whose press
  counts 2, with the pointer source cleared. `auxclick` fires there for the
  release of any non-left button, with the `click` payload. The root tracker
  fires `contextmenu` at the target of a right press, or on macOS a left press
  with Control, right after the `mousedown` input, with
  `EventPayload::context_menu`: `button` 2 and `buttons` 2 (`button` 0 and
  `buttons` 1 for Control), `detail` 0, pointer source mouse. It goes to the
  target of the press's `pointerdown`. On macOS gpui reports a Control+left
  press and its release as right ones without Control. While Control is held
  (`ModifiersChangedEvent` and pointer moves, the `ControlClick` global) the
  tracker turns them back into a left press and release with Control, and a
  left press with Control fires `contextmenu`.
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
  one. Position and modifiers are read at the report. The pointer events
  `pointerout`, `pointerleave`, `pointerover` and `pointerenter` fire first, in
  the same order, and the mouse ones follow. While a capture is active the report
  takes the capturing node as the deepest container, and a gpui hover change
  only updates the hovered set.
- `EventSink::pointer_input` turns each raw mouse input into pointer events
  (`dispatch/pointer.rs`). The held-button mask in `Host::held_buttons` changes
  once per raw `mousedown` or `mouseup`. The first button down fires
  `pointerdown`, the last button up fires `pointerup`, and any other change
  fires `pointermove` with the changed `button`. Each fires before the mouse
  event of the input. `pointermove` fires before `mousemove` and shares its
  movement. A canceled `pointerdown` suppresses `mousedown`, `mousemove` and
  `mouseup` until the `pointerup`, and the press does not focus.
- Pointer capture keeps a pending target (`Host::capture_pending`) and an active
  one (`Host::capture_active`). The bindings change the pending target. The
  next pointer input, hover report or release makes it active: it fires
  `lostpointercapture` at the old target, `document` when that node left the
  tree, then `gotpointercapture` at the new one. Both copy the event that
  caused them. The final button release fires `pointerup`, `mouseup`,
  `lostpointercapture`, `click` or `auxclick` (at the `pointerup` target), then
  the boundary events back to the node under the pointer. Deactivating the
  window fires `lostpointercapture` at the active capture target, then clears
  the held buttons, the capture and a canceled `pointerdown`.

#### Mouse payload

- The payload is `clientX`, `clientY`, `screenX`, `screenY`, `offsetX`,
  `offsetY`, `x`, `y`, `pageX`, `pageY`, `movementX`, `movementY`, `button`,
  `buttons`, `detail`, `relatedTarget` (`null` except on hover events) and the
  four modifier flags.
  `x`/`y` and `pageX`/`pageY` equal `clientX`/`clientY`, and the sink sets
  `screenX`/`screenY` from `Window::bounds` plus the client position.
  The dispatcher sets `offsetX`/`offsetY` from the target's recorded box:
  the client position minus the box origin and border. A target with no
  record gets `clientX`/`clientY`.
- `MousePayload::pointer` adds `pointerId`, `pointerType`, `isPrimary`,
  `width`, `height` and `pressure`: a pointer event is 1, `"mouse"`, true, 1, 1,
  a mouse `click`, `auxclick` and `contextmenu` is 1, `"mouse"`, false, 1, 1,
  and the click of a key is -1, `""`, false, 1, 1. `pressure` is 0.5 on a
  pointer event when `buttons` is non-zero, else 0. `MousePayload::button` is
  `NO_BUTTON` for a pointer event about no button, which reads -1, and `detail`
  is 0 on every pointer event.
- `click`, `dblclick`, `auxclick` and `contextmenu` keep the `buttons` of
  their own payload. Other mouse events take it from `Host::held_buttons`,
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
  focuses the target, or blurs at the end of the order. While no node is
  focused, the move starts from the node of the last mouse press or the last
  focus (`FocusRegistry::press_start`). When that node leaves the tree the
  start moves to the node before its place.
- Enter clicks a focused enabled button on key down. Space clicks it on key
  up with any modifier. An Enter click, a Space key down, a Space key up or
  the button losing focus clears the pending Space. A button under a held
  mouse press also takes one Space click on key up. A click, the mouse
  release or the button losing focus ends the hold.
  A key click uses `MousePayload::keyboard_click`, and `preventDefault()` in
  `keydown` or `keyup` of the `button` ends it.
- The Menu key (`key_name` maps gpui's `"menu"` and the U+0010 of a Mac
  keyboard to `"ContextMenu"`) and Shift+F10 (not on macOS) fire
  `contextmenu` from `key_default` on `keydown`, unless a listener canceled
  it. It targets the focused node with `clientX`/`clientY` at the center of
  its recorded box, or the deepest recorded box at (1, 1) when nothing is
  focused. Its payload is mouse pointer source, `button` `NO_BUTTON` and
  `buttons` 0.
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
  (key compared case-insensitively), `disabled` on a button or input,
  `hidden`, `inert` and `display`, and moving or removing a node, re-check
  the node's subtree before that frame's queued focus requests.
- An uncanceled `mousedown` focuses the nearest node on the target's path
  that has a `FocusRegistry` handle, and blurs the window when there is none.
  The tracker's capture handler calls `Window::prevent_default` to stop gpui's
  focus-on-press.
- A window activation change dispatches `blur` and `focusout`, or `focus` and
  `focusin`, to the focused node. While the window is inactive a later `blur`
  of that node is held back, and a node focused meanwhile receives `focus` and
  `focusin` on activation. `focus` also fires at `"$window"` before the
  focused node on activation, and `blur` after it on deactivation, whether or
  not a node is focused.

#### Text input

- The input element draws the text line inside the container. While the node
  is focused, each paint registers `TextInputHandler` with
  `Window::handle_input`, and the platform's text input and input method call
  it in UTF-16 code units. Reads answer from `Host::inputs`, writes go to
  `EventSink::input_request`.
- `inca_gpui::InputState` holds byte offsets on char boundaries. Each paint
  records the caret stops, the text origin and the sideways scroll
  (`EventSink::record_input`). Mouse hits, `bounds_for_range` and
  `character_index_for_point` read them. After each edit and selection change
  the host calls `Window::invalidate_character_coordinates`, so the platform
  moves the candidate window.
- The default action of an uncanceled `keydown` at a focused input with no
  composition edits it: the arrows, Home and End (Shift extends), Backspace
  by char and Delete by grapheme, and Secondary+A, C, X and V
  (`Modifiers::secondary`). ArrowUp and ArrowDown go to the start and the
  end. Such a key calls `Window::prevent_default` and `App::stop_propagation`.
  Typed characters arrive through the handler after the walk, without
  control characters.
- An uncanceled left `mousedown` on an input puts the caret at the nearest
  stop, and Shift extends the selection. On macOS a Ctrl+click keeps a
  selection it lands in and starts no drag. A move with the left button held
  extends it until the release. Tab into an input selects its text.
- `readonly` and `disabled` stop edits. A `readonly` input still moves its
  selection and copies. With a running composition it applies a selection
  only and leaves the composition open. A script that disables a composing
  input ends it with `compositionend`, then `blur`.
- An edit fires `beforeinput`, applies when no listener canceled it and the
  listeners left the value and the node, then fires `input`. A `beforeinput`
  of `insertCompositionText` is not cancelable (`flags_of`). Backspace or
  Delete with nothing to delete, typing into a `readonly` input and Enter
  fire `beforeinput` only. A paste's `beforeinput` carries the clipboard text
  as read and its `input` the inserted text.
- The first mark of a composition fires `compositionstart` with the selected
  text. Each mark fires `compositionupdate`, `beforeinput` and `input`. A
  commit fires the same, then `compositionend`. An empty mark or an unmark
  ends the composition with `compositionend`. Key events carry `isComposing`
  while the focused input composes.
- `begin_frame` hands each focus transition to the inputs before its events.
  A blurred input ends its composition, then fires `change` when the user
  edited it since its focus or its last `change` and its value differs from
  the value then. A focused input records its value. Enter and window
  deactivation fire `change` the same way and move the recorded value; on
  deactivation `change` comes before the node's `blur`. A script write is no
  edit and leaves the recorded value.

#### Dispatching from script

- The global constructors `Event`, `CustomEvent`, `UIEvent`, `FocusEvent`,
  `MouseEvent`, `PointerEvent`, `WheelEvent`, `KeyboardEvent`, `InputEvent` and
  `CompositionEvent` (`event/construct.rs`, `CLASSES`) build instances of the
  same class with the prototype of their kind, `isTrusted` false and `target`
  `null`. Each needs `new`, a type, and an `init` that is an object, `null` or
  `undefined`. Any other call throws a `TypeError`. Each constructor inherits
  from the constructor of the parent prototype, and `CustomEvent.prototype` from
  `Event.prototype`. The init members convert by their WebIDL type, and a
  non-finite coordinate, delta or size throws a `TypeError`.
- `initEvent()` and `initCustomEvent()` do nothing while the event is
  dispatched. Otherwise they set the type and the two flags, clear the stop
  and cancel flags and `isTrusted`, and keep `composed` and `target`.
- `dispatchEvent` runs the walk in the caller's context, so a listener can
  call it. It sets `target` and `isTrusted`, reads `type` and the flags
  (`bubbles`, `cancelable`, `composed`) from the event and returns the negated
  canceled flag. Default actions and the job queue belong to the caller. The
  walk writes no property of the event object, so a frozen event dispatches
  again.
- `click()` walks a `click` event with `MousePayload::keyboard_click`.
  `Host::clicking` holds the nodes whose click is running, as a re-entrancy
  guard. A disabled `button` or `input` returns at once.

- `HostedApp::render` fires `scroll` after the focus transitions and before
  building the tree. `Host::scrolled_nodes` compares each handle's clamped
  offset with the one recorded last and returns the changed nodes that a
  `scroll` listener reaches: a listener on the node itself, or a capture
  listener on one of its ancestors.

#### Event table

| Name | Payload |
| --- | --- |
| `click`, `auxclick`, `contextmenu` | mouse + pointer |
| `dblclick`, `mousedown`, `mouseup`, `mousemove` | mouse |
| `mouseenter`, `mouseleave`, `mouseover`, `mouseout` | mouse + `relatedTarget` |
| `pointerdown`, `pointerup`, `pointermove`, `gotpointercapture`, `lostpointercapture` | mouse + pointer |
| `pointerover`, `pointerout`, `pointerenter`, `pointerleave` | mouse + pointer + `relatedTarget` |
| `wheel` | mouse + `deltaX`, `deltaY`, `deltaZ`, `deltaMode` |
| `scroll` | none; every flag is false |
| `focus`, `blur`, `focusin`, `focusout` | `relatedTarget` |
| `keydown`, `keyup` | `key`, `repeat`, `isComposing`, modifier flags |
| `beforeinput`, `input` | `data`, `inputType`, `isComposing` |
| `change` | none |
| `compositionstart`, `compositionupdate`, `compositionend` | `data` |

`wheel` negates GPUI's `deltaX` and `deltaY`, `deltaZ` is 0 and `deltaMode` is
0 or 1.

### Timers

The timers come with `incajs`. `packages/core/src/timers.mts` defines them
on load when `__inca_native__` has `startTimer`. The handlers and their
arguments stay in JS, under the id `startTimer` returns. Rust allocates the
ids and keeps them with the due times and periods in `inca_jsenv::Timers`,
which `Host::timers` owns.

`__inca_timer__(id: number) => void` is a global that core defines read-only
on each load. Rust looks it up by name for each due timer and keeps nothing.
A global that is not a function is skipped.

- Core coerces `delay` with `delay | 0` and `max(.., 0)`, refuses a
  non-function handler with a `TypeError`, and throws the `RangeError` "timer
  ids are exhausted. restart the app" when `startTimer` returns `undefined`.
  The clear functions read the id as `id | 0` and call `stopTimer` only for an
  id core holds.
- The hook drops a one-shot entry, then calls the handler with `globalThis`
  as `this` and the stored arguments. It calls `stopTimer` for an id it does
  not hold. A second load of core ends the timers of the first load.
- A timer started in a callback has the callback's nesting level plus 1. A
  timer of level above 6 whose delay is under 4 ms waits 4 ms. The level
  holds while the job queue drains after the callback. Rust keeps it: the
  turn records the level before it calls the hook, and
  `startTimer` reads it.
- A turn (`inca_jsenv::run_timers`) reads the clock once and calls the hook
  for each timer due at that reading in `(due, creation)` order. A repeating
  timer is queued again, at the first point of its schedule (the first due
  time plus whole periods) after that reading, before the hook runs. A timer
  started during the turn waits for the next turn.
- `EventDispatcher::start_timers` runs one foreground task per window. It
  waits on the wake channel and one executor timer, runs a turn when the
  earliest timer is due, and redraws only when a timer ran. `stop_timers`
  ends it. A reload stops the old task before it starts the new one.
- A throw from the hook goes to the error reporter. A listener error the
  callback caused goes to `Host::failures` and is reported after the turn.

## Application menu

The host sets the menu bar (`crates/inca-host/src/menu.rs`). It holds one
menu, titled after the app, with one item, `Quit`, bound to `secondary-q`
(`cmd-q` on macOS, `ctrl-q` elsewhere). `Quit` belongs to the host, so every
app has a way to quit. Only macOS draws a menu bar today.

An item an app defines is dispatched at `window` as the event
`menu:<id>`, through `addEventListener` and the same registry. An item
`save` arrives as `menu:save`.
