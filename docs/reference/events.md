---
description: Every event name Incarnative.js dispatches, the fields it carries, and how propagation and focus work.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Events

A `.vue` template binds a listener the usual way — `@click`,
`@mousedown`, and so on. Every listener receives one shared event
object: `type` (the event's name), `target`/`currentTarget`, the
[base fields](#base-fields), the [propagation methods](#propagation) below,
and whatever fields that event carries.

`currentTarget` is the id of the node whose listener runs. `target` is the
node the event started on:

- `mousedown`, `mouseup`, `mousemove`, `wheel` and `contextmenu` report the
  deepest element under the pointer. Text resolves to its parent element.
- `click`, `dblclick` and `auxclick` report the nearest common ancestor of the elements
  under the press and the release.
- `keydown` and `keyup` report the focused node.
- `mouseover` and `mouseout` report the deepest element the pointer entered or
  left.
- `mouseenter`, `mouseleave`, `focus`, `blur`, `focusin` and `focusout`
  report the node entered, left, focused or blurred.

A listener on an ancestor reads `event.target` to tell which descendant the
event came from. `composedPath()` returns the ids from `target` up to the
root.

```vue
<template>
  <div @mousedown="onMouseDown">Click and hold</div>
</template>
```

## Base fields

Every event carries `bubbles`, `cancelable`, `composed`, `defaultPrevented`,
`eventPhase`, `isTrusted` and `timeStamp`.

- `defaultPrevented` turns `true` when a listener calls `preventDefault()` on
  a cancelable event. Listeners that run later for the same event read it.
  Only `preventDefault()` changes it.
- `eventPhase` is `2` where `currentTarget` is `target` and `3` on the
  ancestors above it.
- `NONE`, `CAPTURING_PHASE`, `AT_TARGET` and `BUBBLING_PHASE` (0 to 3) sit on
  the event object.
- `isTrusted` is `true`. The host produces every event, including a
  `button`'s keyboard `click`.
- `timeStamp` is a number of milliseconds since the host started. Every node
  of one event reads the same value, and a later event reads a larger or equal
  one.
- `srcElement` equals `target`.
- `cancelBubble` reads whether propagation is stopped. Assigning a truthy
  value stops it.
- `returnValue` equals the negation of `defaultPrevented`. Assigning a falsy
  value calls `preventDefault()`.

| Event | `bubbles` | `cancelable` | `composed` |
| --- | --- | --- | --- |
| `click`, `dblclick`, `auxclick`, `contextmenu`, `mousedown`, `mouseup`, `mousemove`, `mouseover`, `mouseout`, `wheel`, `keydown`, `keyup` | `true` | `true` | `true` |
| `mouseenter`, `mouseleave` | `false` | `false` | `false` |
| `focus`, `blur` | `false` | `false` | `true` |
| `focusin`, `focusout` | `true` | `false` | `true` |

An event with any other name has all three set to `false`.

> [!NOTE]
> These are the only event names wired to real input. Binding any other
> name is accepted but never fires.

## Pointer

The mouse events carry a subset of the DOM's
[`MouseEvent`](https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent)'s
fields:

- `clientX` / `clientY` — pointer position
- `x` / `y` — identical to `clientX`/`clientY`
- `pageX` / `pageY` — identical to `clientX`/`clientY`; nothing here
  scrolls the page itself, which is the only thing that would tell them
  apart
- `movementX` / `movementY` — how far the pointer moved:
  - `mousemove`, and `mouseover`, `mouseout`, `mouseenter` and `mouseleave`
    caused by a pointer move: the distance since the previous pointer move,
    `0` for the first move.
    Every event from one move reports the same value
  - every other event: the event position minus the position of the last
    pointer move
  - After the pointer leaves the window, the next move reports `0`
- `button` — the button the event is about; `0` for a move, which isn't
  about any one button
- [`buttons`](https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/buttons) —
  every button currently held, as a bitmask
- `detail` — how many clicks this is part of; `0` for a move
- `relatedTarget` — the node on the other side of a hover change, see
  [the hover events](#mouseenter-mouseleave-mouseover-mouseout). `null` on
  every other mouse event
- `ctrlKey` / `shiftKey` / `altKey` / `metaKey` — modifier keys held

### `mousedown` / `mouseup` / `mousemove`

The mouse fields above.

### `mouseenter` / `mouseleave` / `mouseover` / `mouseout`

`mouseover`, `mouseout`, `mouseenter` and `mouseleave` fire when the deepest
element under the pointer changes.

- `mouseout` fires first at the element the pointer left, then `mouseleave`
  at each element the pointer left, innermost first. `mouseover` fires at the
  element the pointer entered, then `mouseenter` at each element it entered,
  outermost first.
- `mouseover` and `mouseout` bubble. A pointer moving into `C` inside `B`
  inside `A` fires `mouseover` on `C`, `B` and `A`, then `mouseenter` on `A`,
  `B` and `C`. Each `mouseenter` and `mouseleave` runs only the listeners of
  its own element, and `target` is that element.
- `relatedTarget` is the element the pointer left for `mouseover` and
  `mouseenter`, and the element it entered for `mouseout` and `mouseleave`.
  It is `null` when the pointer comes from or goes outside the window.
- The `mousemove` of the same pointer move fires after these events.
- The other fields are the mouse fields above.

An element already under the pointer when it mounts, or one a layout change
puts under a still pointer, fires these events the next time its hover state
is checked. They report `0` for `movementX`/`movementY` and `null` for
`relatedTarget` when no element was hovered before.

### `click`

Fires when the primary button is pressed and released on the same node, before
the `mouseup` listeners run. Only the primary button fires `click`. The right
button fires [`contextmenu`](#contextmenu). The right and middle buttons fire
[`auxclick`](#auxclick).

`click` carries the mouse fields above, with `button` and `buttons` at `0`,
`detail` the click count and the position and modifier keys of the release. It
also carries a subset of the DOM's
[`PointerEvent`](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent)'s
fields: `pointerId` `1`, `pointerType` `"mouse"`, `isPrimary` `true`, `width`
`1`, `height` `1` and `pressure` `0`.

A [focused](#focus) [`button`](./elements#button) also fires `click` from the
keyboard. `Enter` fires it when the key goes down and again for each repeat
while the key is held. `Space` fires it when the key goes up. `Ctrl`, `Alt`
or `Meta` held with `Enter` cancels the click. `Shift` allows it. Only a
button clicks from `Enter` and `Space`.

A keyboard click bubbles from the button to its ancestors, with the button as
`target`. It carries `detail`, `button`, `buttons`, `clientX`, `clientY` and
`pressure` at `0`, `pointerId` `-1`, `pointerType` `""`, `isPrimary` `false`,
`width` and `height` `1`, and the modifier keys.

A button with the `disabled` attribute is unfocusable. Its `mousedown`,
`mouseup` and `click` end at the button: listeners on its descendants run, and
listeners on the button and its ancestors stay silent. `mouseenter`,
`mousemove` and `wheel` reach every listener as usual. `contextmenu` and
`auxclick` reach the button's ancestors with the button as `target`. A press
on a disabled button moves focus as a press on any other node does.
`:disabled="false"` removes the attribute.

### `dblclick`

Fires on the node after the second `click` of a double click of the primary
button. It carries the mouse fields above with `button` and `buttons` at `0`
and `detail` `2`. A third click fires only `click`, with `detail` `3`.

### `auxclick`

Fires when the right or middle button is pressed and released on the same
node. It carries the same fields as `click`, with `button` `2` or `1`.

### `contextmenu`

Fires on a press of the right button, after the `mousedown` listeners of every
node have run, whether or not one of them called `stopPropagation()`. The
`target` is the deepest element under the pointer.

`contextmenu` carries the mouse fields above with `button` `2`, `buttons` `2`
and `detail` `0`, and the `PointerEvent` fields of a mouse `click`. The
`mouseup` follows it. `preventDefault()` only sets `defaultPrevented`.

## Wheel

### `wheel`

The same fields as `mousedown`, above, plus
[`WheelEvent`](https://developer.mozilla.org/en-US/docs/Web/API/WheelEvent)'s:

- `deltaX` / `deltaY` — scroll distance; positive when scrolling down or
  right, as in the DOM
- `deltaZ` — always `0`
- `deltaMode` — `0` (pixels) or `1` (lines), never `2` (page)

In a scrolling container, `stopPropagation()` keeps ancestors' `wheel`
listeners from firing, and the container still scrolls. `preventDefault()`
cancels the scroll. Nested containers scroll the innermost first, and the
wheel moves to the next container out once the inner one is at its limit.

The `scroll` event is not supported.

## Keyboard

### `keydown` / `keyup`

Fire on whichever node is [focused](#focus), bubbling to its ancestors. With
nothing focused, they fire on the root node, which is the `target`. The fields
are a subset of the DOM's
[`KeyboardEvent`](https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent):

- `key` — DOM-named where recognized, the typed character otherwise, and
  `"Unidentified"` when the platform reports none
- `repeat` — always `false` for `keyup`
- `location` — always `0`
- `isComposing` — always `false`
- `ctrlKey` / `shiftKey` / `altKey` / `metaKey` — modifier keys held
- `getModifierState(name)` — whether `"Control"`, `"Shift"`, `"Alt"`, `"Meta"`
  or `"Accel"` is held. `"Accel"` is the virtual accelerator modifier, `Meta`
  on macOS and `Control` elsewhere. Other names return `false`. Mouse events
  carry it as well.

The fields above are the supported set. `keydown` and `keyup` are the only key
events.

## Focus

A node takes focus through its `tabindex` attribute. Any integer works,
including a negative one. A text node ignores `tabindex`. So does a node that
has `display: none`, `inert` or `hidden` on itself or on any ancestor. `hidden`
renders the node as `display: none` unless the node sets its own `display`.
`hidden="until-found"` is treated as unhidden. An `inert` subtree receives no
mouse events and never becomes `event.target`. Wheel scrolling of an `inert`
scroll container still works.

A click on a node with `tabindex` focuses it, and so does `.focus()`.
`.focus()` on any other node is ignored. A click on a plain child focuses the
nearest ancestor with `tabindex`. A click on an area outside every such node
blurs the focused node. `preventDefault()` in a `mousedown` handler cancels
both the focus change and the blur.

A [`button`](./elements#button) has a default `tabindex` of 0.

A change of any of these applies at once. When the node becomes unfocusable, it
leaves focus and the host fires `blur` and `focusout`. `el.tabIndex = n` sets
the attribute.

`.blur()` unfocuses the node when it is the focused one.

A template `ref`'s own element carries `.focus()`/`.blur()` directly,
the same as a real DOM element:

```vue
<script setup>
import { onMounted, ref } from "vue";

const input = ref(null);
onMounted(() => input.value.focus());
</script>

<template>
  <div ref="input" tabindex="0" @focus="onFocused" @blur="onBlurred">...</div>
</template>
```

A focus change fires `blur`, `focusout`, `focus` and `focusin`, in that
order. `focusin` and `focusout` bubble, so a listener on an ancestor sees the
focus changes of its descendants. `focus` and `blur` stay on the node.

`relatedTarget` is the id of the node on the other side of the change, or
`null` when there is none. `blur` and `focusout` carry the node gaining
focus. `focus` and `focusin` carry the node losing it. The four events carry only `relatedTarget`.

## Propagation

An event bubbles from the node it fired on up through its ancestors,
the same as the DOM's. Every listener receives the same three methods to
change that:

```vue
<script setup>
function onClick(event) {
  event.stopPropagation();
}
</script>
```

- [`stopPropagation()`](https://developer.mozilla.org/en-US/docs/Web/API/Event/stopPropagation) /
  [`stopImmediatePropagation()`](https://developer.mozilla.org/en-US/docs/Web/API/Event/stopImmediatePropagation) —
  work exactly as the DOM's.
- `preventDefault()` — suppresses a native default action: [focus](#focus)'s
  own click-to-focus, a [`button`](./elements#button)'s `click` from
  `Enter` (in `keydown`) or `Space` (in `keyup`), or a [`wheel`](#wheel)'s
  scrolling.

  `preventDefault()` takes effect on a cancelable event only.

Every listener runs in the bubble phase; there's no way yet to listen
during the capture phase.

A listener added while an event is being handled first runs for the next event.

## Event modifiers

Templates accept Vue's event modifiers. Each one acts on the host's events as
described here.

> [!NOTE]
> Some modifiers cannot reproduce Vue's behavior yet. The sections below
> describe each difference.

### Calls and listeners

- `.stop` and `.prevent` call `stopPropagation()` and `preventDefault()`.
- `.once` removes the listener after its first call.
- `.self` runs the handler when `target` is `currentTarget`.
- `.passive` and `.capture` bind an ordinary listener. It runs in the bubble
  phase like every listener, see [Propagation](#propagation).

### Modifier keys

`.ctrl`, `.shift`, `.alt` and `.meta` run the handler while that key is held.
`.exact` runs it when the keys held are exactly the ones listed.

- `keydown`, `keyup`, `click`, `dblclick`, `auxclick`, `contextmenu`, `mousedown`,
  `mouseup`, `mousemove`, `mouseenter`, `mouseleave` and `wheel` carry the key
  state.
- `click` and `dblclick` carry the key state of the release.

### Mouse buttons

`.left`, `.middle` and `.right` compare the event's `button` with 0, 1 and 2,
for example `@mousedown.left`.

- `@click.left` runs on every `click`, which carries `button` `0`.
- `@click.middle` listens for `mouseup` and runs when the released button is
  1.
- `@click.right` listens for `contextmenu` and runs for the right button's
  press.

### Keys

On `keydown` and `keyup`, `.enter`, `.tab`, `.esc`, `.space`, `.up`, `.down`,
`.left` and `.right` compare the `key` field, as in `@keydown.enter`. Write
other keys in kebab-case: `@keydown.page-up` matches `PageUp`.

```vue
<template>
  <div @click.stop="onClick" @keydown.enter="onEnter">...</div>
</template>
```

## Errors in handlers

An error an `@event` handler throws, or a rejection of an `async` handler,
goes to any `onErrorCaptured` hooks and then to `app.config.errorHandler`. A
hook that returns `false` stops it there. Every handler of an event runs even
when an earlier one fails, until one calls `stopImmediatePropagation()`.

An error that no hook or `errorHandler` takes follows Vue's unhandled-error
logging. A production build logs it to the console. A development build warns
and reports each failing handler to the host as an application error. With
`app.config.throwUnhandledErrorInProduction` set, a production build throws
the error to the host, which reports it.
