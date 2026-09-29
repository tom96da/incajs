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
[propagation methods](#propagation) below, and whatever fields that event
carries.

`target` and `currentTarget` are both the id of the node currently
receiving the event — they're always equal, since there's no way yet to
learn which descendant an event actually started on (no event
delegation).

```vue
<template>
  <div @mousedown="onMouseDown">Click and hold</div>
</template>
```

> [!NOTE]
> These are the only event names wired to real input. Binding any other
> name is accepted but never fires.

## Pointer

### `click`

Fires after a `mousedown` and `mouseup` on the same node, in that
order — or after `Enter`/`Space` while the node is [focused](#focus).
Carries no fields of its own.

### `mousedown` / `mouseup` / `mousemove`

A subset of the DOM's
[`MouseEvent`](https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent)'s
fields:

- `clientX` / `clientY` — pointer position
- `pageX` / `pageY` — identical to `clientX`/`clientY`; nothing here
  scrolls the page itself, which is the only thing that would tell them
  apart
- `movementX` / `movementY` — delta from whichever mouse/wheel event
  fired last; `0` for the first one
- `button` — 0 for a move, which isn't about any one button
- [`buttons`](https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/buttons) —
  every button currently held, as a bitmask
- `detail` — how many clicks this is part of; 0 for a move
- `ctrlKey` / `shiftKey` / `altKey` / `metaKey` — modifier keys held

### `mouseenter` / `mouseleave`

Same fields as `mousedown`, above. An element already under the pointer
when it mounts gets a `mouseenter` the first time its hover state is
checked, with no pointer movement involved — unlike the DOM, where
`mouseenter` only ever follows an actual move. Its `movementX`/`movementY`
share the same tracker every other mouse event does, so hovering with no
pointer movement since an earlier click can still report a nonzero delta,
against that click's position.

## Wheel

### `wheel`

The same fields as `mousedown`, above, plus
[`WheelEvent`](https://developer.mozilla.org/en-US/docs/Web/API/WheelEvent)'s:

- `deltaX` / `deltaY` — scroll distance; positive when scrolling down or
  right, as in the DOM
- `deltaZ` — always `0`
- `deltaMode` — `0` (pixels) or `1` (lines), never `2` (page)

## Keyboard

### `keydown` / `keyup`

Fire on whichever node is [focused](#focus), bubbling to its ancestors —
with nothing focused, neither reaches any node. A subset of the DOM's
[`KeyboardEvent`](https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent)'s
fields:

- `key` — DOM-named where recognized, the raw character otherwise
- `repeat` — always `false` for `keyup`
- `ctrlKey` / `shiftKey` / `altKey` / `metaKey` — modifier keys held

There's no `code`, `location`, or `isComposing` yet.

## Focus

A node isn't focusable until `.focus()` has been called on it at least
once — clicking a node that's never been focused this way doesn't move
focus there, unlike a DOM element with a `tabindex`.

Once a node has been focused this way, it stays focusable from then on:
clicking it moves focus there on its own, the same as a focusable DOM
element does. `.blur()` unfocuses whatever is currently focused.

A template `ref`'s own element carries `.focus()`/`.blur()` directly,
the same as a real DOM element:

```vue
<script setup>
import { onMounted, ref } from "@vue/runtime-core";

const input = ref(null);
onMounted(() => input.value.focus());
</script>

<template>
  <div ref="input" @focus="onFocused" @blur="onBlurred">...</div>
</template>
```

`focus`/`blur` carry no fields of their own.

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
  own click-to-focus, or a focused node's `click` firing from
  `Enter`/`Space`.

Every listener runs in the bubble phase; there's no way yet to listen
during the capture phase.

## Event modifiers

Vue's template event modifiers — `v-on:click.once`, `.passive`,
`.capture` — are Vue's own syntax, not a web standard.

- `.once` is fully real: the listener unbinds after its first call.
- `.passive` and `.capture` are accepted but currently degrade to an
  ordinary bubble-phase listener, for the same reason given in
  [Propagation](#propagation) above — Incarnative.js has no capture-phase
  dispatch yet, so `.capture` doesn't run during the capture phase, and
  `.passive` has no effect beyond a plain bind.
