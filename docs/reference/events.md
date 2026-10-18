---
description: The events Incarnative.js dispatches, the web standards they follow, and where the host differs.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Events

A `.vue` template binds a listener the usual way, such as `@click` or
`@keydown`. Every listener receives one event object with the fields and
methods of the DOM `Event` interface (see Base fields).

```vue
<template>
  <div @mousedown="onMouseDown">Click and hold</div>
</template>
```

## Event targets

`window` from `incajs` (`import { window } from "incajs"`) and a template `ref`
have `addEventListener()`, `removeEventListener()` and `dispatchEvent()`.

- `composedPath()` lists the app's elements, then `window`, with no `body`,
  `html` or `document`. `document` is unsupported.
- The `signal` option of `addEventListener()` is unsupported.
- The ref of an unmounted element ignores `addEventListener()` and an
  assignment to a handler property, and `dispatchEvent()` returns `true`.
- An attribute that holds code, such as `onclick="count++"`, runs nothing.
- `window` has no handler properties such as `onkeydown`.

## Event modifiers

Templates accept the
[event modifiers](https://vuejs.org/guide/essentials/event-handling#event-modifiers)
and the
[key modifiers](https://vuejs.org/guide/essentials/event-handling#key-modifiers)
of Vue.

## Base fields

Every event object carries the fields of the DOM
[`Event`](https://dom.spec.whatwg.org/commit-snapshots/b76da7af5fe35a2a368127cc9ec22449ac8fd559/#interface-event)
interface. This includes the legacy aliases `srcElement`, `cancelBubble`
and `returnValue`. The three flags depend on the event:

| Events                           | bubbles | cancelable | composed |
| -------------------------------- | ------- | ---------- | -------- |
| mouse, wheel and keyboard events | yes     | yes        | yes      |
| `mouseenter`, `mouseleave`       | no      | no         | no       |
| `scroll`                         | no      | no         | no       |
| `focus`, `blur`                  | no      | no         | yes      |
| `focusin`, `focusout`            | yes     | no         | yes      |

- `timeStamp` counts milliseconds since the host started.
- Removing the focused node, or moving it within the tree, leaves `blur` and
  `focusout` unfired.

## Mouse

Mouse events carry the fields of
[`MouseEvent`](https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent)
from
[Pointer Events](https://www.w3.org/TR/2026/WD-pointerevents4-20261005/#mouse-event-types).
Every mouse event type of that standard fires, except the pointer events (see
Host differences). `wheel` carries the fields of
[`WheelEvent`](https://developer.mozilla.org/en-US/docs/Web/API/WheelEvent)
from [Pointer Events](https://www.w3.org/TR/2026/WD-pointerevents4-20261005/#wheel-event-types).

`click`, `auxclick` and `contextmenu` carry the fields of
[`PointerEvent`](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent)
with fixed values, since the host has a single mouse pointer and no pointer
events yet. `pointerType` is `"mouse"`, or `""` for the click of a key.

| Event         | button | buttons | detail      |
| ------------- | ------ | ------- | ----------- |
| `click`       | 0      | 0       | click count |
| `dblclick`    | 0      | 0       | 2           |
| `auxclick`    | 1 to 4 | 0       | click count |
| `contextmenu` | 2      | 2       | 0           |

### Host differences

- `preventDefault()` in a `contextmenu` listener only sets `defaultPrevented`.
- `screenX` and `screenY` add the window position. On Wayland they equal the
  client position.
- Pointer events (`pointerdown` and the rest) are unsupported.
- `scrollend` is unsupported.
- A mouse event outside every element has `window` as `target`, where browsers
  report `body` (Firefox reports `html`). Moving the pointer between an element
  and an empty area gives `window` as `relatedTarget`.

## Keyboard

Keyboard events carry the fields of
[`KeyboardEvent`](https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent)
from [UI Events](https://www.w3.org/TR/2026/WD-uievents-20260221/#events-keyboard-types).
`keydown` and `keyup` go to the focused node, or to `window` when nothing is
focused. `target` is that node or `window`, where browsers report `body`. A
`window` listener reads `eventPhase` 2 for a key event while nothing is
focused.

| Field         | Value                                           |
| ------------- | ----------------------------------------------- |
| `key`         | `"Unidentified"` when the platform reports none |
| `repeat`      | `false` on `keyup`                              |
| `location`    | 0                                               |
| `isComposing` | `false`                                         |

- `code` is not set. `key` identifies the key.
- `getModifierState()` accepts Control, Shift, Alt, Meta and Accel. Accel is
  Meta on macOS and Control elsewhere.
- Pressing a modifier alone fires `keydown` and `keyup` with its name as
  `key`.
- The quit shortcut (Cmd+Q on macOS, Ctrl+Q elsewhere) goes to the host only.

## Focus

Focus events carry the fields of
[`FocusEvent`](https://developer.mozilla.org/en-US/docs/Web/API/FocusEvent)
from [UI Events](https://www.w3.org/TR/2026/WD-uievents-20260221/#events-focus-types).
Focus and Tab navigation follow the
[HTML `tabindex`](https://html.spec.whatwg.org/commit-snapshots/ba3400bb4f3d86477a5c8cfc07ff91fc0caaf21c/#attr-tabindex)
rules, with the differences below.

### Host differences

- A node takes focus through its `tabindex` attribute, and a `button` is
  focusable by default. `.focus()` on other nodes is ignored.
- `hidden="until-found"` counts as shown.
- A focus ring is unsupported.
- `activeElement()` (`import { activeElement } from "incajs"`) returns the
  focused element or `null`. It replaces `document.activeElement`, which is
  unsupported.
- After the last node, Tab leaves nothing focused and the next Tab starts at
  the first node, as in Chromium and WebKit. Focus stays in the window.
- Tab after a click starts at the first node, where browsers continue from the
  click position.

## Dispatching events

`dispatchEvent()` sends an event that a script builds with `Event` or
`CustomEvent`.

- `MouseEvent`, `KeyboardEvent`, `FocusEvent`, `WheelEvent`, `UIEvent` and
  `PointerEvent` are unsupported, as is `initEvent()`.
- Dispatching an event again after a listener froze it throws a `TypeError`.

## Errors in handlers

An error a handler throws, or an `async` handler rejects with, follows Vue's
[error handling](https://vuejs.org/api/composition-api-lifecycle#onerrorcaptured).

The development window reports an error that no hook takes to the terminal
that ran `inca dev`, and shows it as an application error. A packaged app
writes it to stderr.

> [!TIP]
> A packaged app has no readable stderr. Install `app.config.errorHandler` to
> show these errors yourself.
