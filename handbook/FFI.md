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
| `addEventListener` | `(nodeId: number, event: string, callbackId: number, capture?: boolean, passive?: boolean) => void` | Register a callback for an event. Ids stack per `(nodeId, event)`, see [Registering](#registering). `capture` and `passive` default to `false`. |
| `removeEventListener` | `(nodeId: number, event: string, callbackId: number) => boolean` | Drop one registration and return whether it was there. An unknown node returns `false`. |
| `dispatchEvent` | `(nodeId: number, event: Event) => boolean` | Walk a script-built event over the path of `nodeId` and return `false` when a listener canceled it, see [Dispatching from script](#dispatching-from-script). |
| `click` | `(nodeId: number) => void` | Walk an untrusted `click` at `nodeId`, see [Dispatching from script](#dispatching-from-script). |
| `destroyNode` | `(nodeId: number) => number[]` | Free `nodeId` and its subtree, and return every `callbackId` registered in it. An unknown id returns `[]`. Destroying the root throws. Also clears the focus state of the destroyed nodes. |
| `focusNode` | `(nodeId: number) => void` | Request focus for `nodeId`. Takes effect next frame. A node with a `tabindex`, and only that node, takes focus. |
| `blurNode` | `(nodeId: number) => void` | Request that `nodeId` lose focus. Takes effect next frame, and does nothing if `nodeId` is not the focused node by then. |

Shared throw rules, all as a `TypeError`:

- A malformed `nodeId` or `callbackId` throws.
- `setAttribute` and `setStyle` throw for a `value` that is not a string, number or boolean.
- `dispatchEvent` throws for an argument that is not an `Event` or `CustomEvent` of the host.
- `addEventListener`, `dispatchEvent`, `setAttribute`, `removeAttribute`, `setStyle`, `removeStyle` and `appendChild` throw for an unknown node. `insertBefore` throws for an unknown parent, child or anchor.
- `removeChild` throws for an unknown `parentId` and ignores an unknown or unattached `childId`.

`dispatchEvent` also throws an `Error` named `InvalidStateError` for an event that is being dispatched. `click` ignores an unknown node.

`insertBefore` and `appendChild`:

- If `anchorId` equals `childId` and the child is already under `parentId`,
  nothing changes.
- If `anchorId` is a real node that is not a child of `parentId`, the child
  is appended at the end.
- An `anchorId` that names no node throws.
- A `childId` naming the root throws.

The host converts the `VirtualNode` tree into GPUI elements on each frame.
While a listener is registered or a node can take focus, every container that
is not inert reports the pointer (`EventSink::tracks_pointer`).

### Event dispatch

The host dispatches native input to the JS callbacks registered for a node
and event name. Payloads and author-facing behaviour are in
[events.md](../docs/reference/events.md).

#### Registering

- `addEventListener`/`removeEventListener` are in the binding table above.
  Distinct `callbackId`s stack on one `(nodeId, event)`. Adding an id again
  is a no-op and keeps its first `capture` and `passive`.
- `capture` and `passive` belong to the registration. A callback id belongs to
  one registration, so `removeEventListener` takes the id only.
- The caller stores the function at `globalThis.__inca_callbacks__[callbackId]`
  before registering. The host stores only the `u32` id.
- A missing or non-function entry is skipped.
- A throwing callback, or a promise rejected with no handler, goes to the
  host's error reporter. Sibling callbacks still run.
- `removeEventListener` and `destroyNode` return the dropped ids, because
  only the caller can free the JS half.
- A dynamically-named event, such as `menu:<id>`, uses the same registry and
  is fired by the host.

#### Event object

- A walk passes one object to every listener on the path. The host creates
  it for its own events, and a script builds it for the events it dispatches.
  Its own fields are `type`, `target`, `currentTarget`, `eventId` and the
  payload. A property one listener sets is visible to the listeners after it.
  `currentTarget` is the node whose listeners run. Once the walk ends,
  `currentTarget` is `null`, `eventPhase` is 0, `composedPath()` returns an
  empty array and the stop flags are clear. `defaultPrevented` keeps its value.
- The object inherits from a prototype that the host creates once per
  engine. The prototype holds `stopPropagation`, `stopImmediatePropagation`,
  `preventDefault`, `composedPath`, the accessors `defaultPrevented`,
  `returnValue`, `cancelBubble` and `srcElement`, and the phase constants.
  They read and write a state record kept in a `WeakMap` that lives in the
  engine's userdata (`crates/inca-bridge/src/event.rs`) and is private to the
  host. A call whose receiver has no record, such as a detached
  `const p = e.preventDefault; p()`, throws a `TypeError`.
- The target of a pointer event (`mousedown`, `mouseup`, `mousemove`,
  `wheel`, `contextmenu`) is the deepest container under the pointer, or the
  root when there is none. The mouse target slot is a gpui global in
  `inca-gpui`. The root's pointer tracker clears it in the capture phase of
  each mouse event. While the sink tracks the pointer, every container that
  is not inert adds a marker listener, and the first marker to run, the
  deepest container, keeps the slot. The tracker is the first child of the
  root, so its bubble handlers run after every container below the root. It
  reports each input once through `EventSink::pointer_input`. A press also
  records its own container per button, kept until a release of that button
  has been reported. A press of another button leaves it. The window losing
  activation clears every recorded press (`inca_gpui::clear_presses`).
- The target of `keydown` and `keyup` is the focused node, or the root when
  nothing is focused. Every other event takes the node it is fired at.
- `eventId` is a number that grows with each event. All listeners of one walk
  carry the same value. Only an event with a listener it reaches enters the
  engine, drains jobs, requests a redraw and takes a number. One engine has
  one counter (`Host::last_event_id`), shared by all its dispatchers, and it
  restarts at 1 with a new engine.
- Base fields: `bubbles`, `cancelable` and `composed` come from one table
  (`EVENTS` in `dispatch.rs`, which also holds each event's movement rule
  and own-buttons rule; other names get every rule `false`). The `timeStamp`
  origin is a process-wide `Instant` fixed when the first dispatcher is
  created. `defaultPrevented` is set by a `preventDefault()` call on a
  cancelable event, and later nodes of the walk read it as `true`.
- Accessors: `defaultPrevented` (getter), `returnValue` and `cancelBubble`
  (getter and setter with ToBoolean coercion) read the flags the methods set.
  `srcElement` reads `target`. The phase constants are non-enumerable data
  properties of the prototype and of `Event`.
- GPUI's `platform` modifier becomes `metaKey`. `function` is dropped.
  `getModifierState` exists on key and mouse events and reads those four.
  `Accel` reads `platform` on macOS and `control` elsewhere.
- With nothing focused, the window focus rests on a handle the root container
  tracks (`FocusRegistry::parked_handle`, mapped to no node). The root's key
  listeners run and `target` is the root. The root wires `keydown` and
  `keyup` even without a listener. `key` is `"Unidentified"` for an empty
  key, and a letter with Shift held and no `key_char` is upper case.

#### Dispatching from script

- `event.rs` installs the globals `Event` and `CustomEvent` with
  `Constructor::new_prototype`. Their events get the shared prototype and a
  record in the same `WeakMap`, with `isTrusted` false and `target` `null`.
  A call without `new` (`Params::is_constructor`), a missing type and an init
  that is neither `null`, `undefined` nor an object throw a `TypeError`. `CustomEvent.prototype` inherits from
  `Event.prototype`.
- `dispatchEvent` runs the walk inside the caller's context, so a listener
  can call it. It sets `target` and `isTrusted`, takes a new `eventId` when a
  listener is reached, reads `type` and the flags from the record, and returns
  the negated canceled flag. Default actions and the job queue belong to the
  caller. The `dispatching` flag is set last and cleared before the event
  object is touched, so a frozen event stays dispatchable.
- `click` walks `click` with `MousePayload::keyboard_click`. `Host::clicking`
  holds the nodes whose click is running. A disabled `button` and an unknown
  node return at once.
- Listener errors of every walk go to `Host::failures` as they happen.
  The dispatcher reports them in that order after each walk and after
  `drain_jobs_and_refresh`, and the development window's load poll reports
  them with `EventDispatcher::report_pending`.

#### Events

| Name | Payload | Notes |
| --- | --- | --- |
| `click` | mouse + pointer | Fires after `mouseup` at the nearest common ancestor of the press container of that button and the release container, so a press must be recorded. A release over no container uses the root. The payload is `EventPayload::click`: the release position and modifiers, `button` the released button, `buttons` at 0, `detail` the click count of the press, pointer source mouse. A key click fires from `EventSink::key_input` at the focused enabled button with `MousePayload::keyboard_click`. |
| `dblclick` | mouse | Fired through `EventSink::fire` at the `click` ancestor right after the `click` whose press counts 2, with the pointer source cleared. |
| `auxclick` | mouse + pointer | Fired through `EventSink::fire` at the same ancestor for the release of any button but the left one. The payload is `EventPayload::click` with `button` the released button. |
| `contextmenu` | mouse + pointer | The root tracker fires `contextmenu` through `EventSink::fire` for a right `MouseDownEvent`, right after the `mousedown` input and at its target. The payload is `EventPayload::context_menu`: `button` 2, `buttons` 2, `detail` 0, pointer source mouse. |
| `mousedown`, `mouseup` | mouse | `buttons` holds every button currently held. |
| `mousemove` | mouse | `movementX`/`movementY` hold the delta of the raw move. |
| `mouseenter`, `mouseleave` | mouse + `relatedTarget` | Does not bubble. |
| `mouseover`, `mouseout` | mouse + `relatedTarget` | `EventDispatcher::fire` runs the walk from the deepest container. |
| `wheel` | mouse + `deltaX`, `deltaY`, `deltaZ`, `deltaMode` | `deltaX`/`deltaY` are GPUI's values negated. `deltaZ` is `0`. `deltaMode` is `0` or `1`. |
| `focus`, `blur` | `relatedTarget` | Does not bubble. |
| `focusin`, `focusout` | `relatedTarget` | `EventDispatcher::fire` runs the walk with `target` the node. A focus change dispatches `blur`, `focusout`, `focus`, `focusin`. |
| `keydown`, `keyup` | `key`, `repeat`, `location` (0), `isComposing` (false), `ctrlKey`, `shiftKey`, `altKey`, `metaKey`, `getModifierState` | Go to the focused node, or the root when none is focused, and bubble to its ancestors. `keyup` has `repeat: false`. |

While the sink tracks the pointer (`EventSink::tracks_pointer`), every
non-inert container wires one `on_hover`, which calls
`EventSink::hover_changed` and keeps the hovered set. The first container to
report in a pointer move calls `EventSink::pointer_over` with itself as the
deepest one, which reports at once. Any other change (a layout change under a
still pointer, a window exit) reports once at the end of the update from the
deepest container of the hovered set. A report compares the deepest container
with the previous one (`EventDispatcher::hover_prev`) and fires, per
`hover_path_difference`: `mouseout` at the old container, `mouseleave` on each
container left (innermost first), `mouseover` at the new one, `mouseenter` on
each container entered (outermost first). `relatedTarget` of the first two is
the new container and of the last two the old one. A removed old container
receives no `mouseout` or `mouseleave`, and its nearest remaining ancestor is
the old one. Position and modifiers are read from the window when the report
runs.

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

- A walk has two passes. The capture pass runs the `capture` callbacks of
  each node from the root down to the target, with `eventPhase` 1 on the
  ancestors and 2 on the target. The bubble pass then runs the other callbacks
  from the target up through the ancestors while the event bubbles, with
  `eventPhase` 2 on the target and 3 on the ancestors. An event that does not
  bubble runs the capture pass over the whole path and the bubble pass on the
  target only. At the target, capture callbacks run before the others.
- The path is fixed when the walk starts. A node's listeners are the ones
  registered when the walk reaches it, minus any removed since. A listener
  added to a node during that node's callbacks of a pass skips that pass.
  The node's other pass includes it, when the event reaches that pass.
- `stopPropagation()` lets the remaining callbacks of that node in the current
  pass run and gives every later node and pass none. The target's capture
  callbacks and its bubble callbacks are separate passes, so a stop in the
  first skips the second. `stopImmediatePropagation()` also stops the
  remaining callbacks of that node. Both act on the walk and leave gpui's own
  propagation running, so hover, scrolling and every other gpui listener run
  as they do for any input.
- `EventDispatcher::fire` and the pointer walks drop the callbacks of
  `mousedown`, `mouseup`, a mouse `click` and `dblclick` for a disabled
  button and its ancestors. A key click always runs. Listeners below the
  button run.
- `preventDefault()` marks a cancelable event unless the running callback is
  passive. A passive callback can still stop propagation. The default action
  is the focus change or the blur of a `mousedown`, wheel scrolling (the
  scroll rolls back), and the Tab, Enter and Space actions of a key. For a
  `button`, `preventDefault()` in `keydown` or `keyup` also ends the key
  click.
- A canceled `keydown` or `keyup` calls `Window::prevent_default` and
  `App::stop_propagation` once its walk ends. The platform then treats the key
  as handled (for example macOS `performKeyEquivalent`) and later key
  listeners of gpui stay idle. A stopped or unlistened key stays unhandled.
  Keybindings run before key listeners, so a bound action consumes its key
  before any walk.

#### Ordering

- `focusNode`/`blurNode` queue and apply in order on the next frame.
- The host reads `tabindex` on `setAttribute`/`removeAttribute` (key
  compared case-insensitively), `disabled` on a button, and `hidden`,
  `inert` and `display` changes, which re-check the node's subtree. Moving a
  node re-checks its subtree too.
  The host applies the change before the queued focus requests of that
  frame. `build_spec` marks an `inert` node and its subtree `inert`. They stay
  outside hit testing and hover. The user rules are in
  `docs/reference/events.md`.
- A capture-phase `key_down` and `key_up` listener on the root container
  calls `EventSink::key_input`, which walks `keydown` or `keyup` and then runs
  the default action when the event is not canceled. `Tab` and `Shift+Tab`
  move focus (Control, Alt and Meta cancel the move). Enter clicks a focused
  enabled button on key down. Space records the button in `space_down` and
  clicks on key up with any modifier. Another key down, a mouse press
  (`EventSink::pointer_pressed`) or a Space key up clears the pending Space.
  `FocusRegistry::tab_move` collects the handled nodes in tree order and
  focuses the target, or blurs for the end of the order. `apply_pending`
  reports the transition on the next frame.
- The root tracker's bubble handler for a `MouseDownEvent` reports the
  `mousedown` input. Its capture handler calls `Window::prevent_default`, which
  stops gpui's own focus-on-press from running. After the walk, a `mousedown`
  that is not canceled focuses the nearest node on the target's path that has
  a `FocusRegistry` handle, and blurs the window when there is none.
- The root tracker's bubble handler for a `MouseUpEvent` reports the `mouseup`
  input and then calls `EventSink::pointer_released`. The click events fire
  after that `mouseup`, then the press of that button is forgotten.
- Every raw `mousedown` and `mouseup` updates the held buttons. `mousemove`,
  `wheel` and the other events without own buttons read them.
- Destroying a focused node fires no `blur`.
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
