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

`document` and `window` from `incajs` (`import { document, window } from
"incajs"`) and a template `ref` have `addEventListener()`,
`removeEventListener()` and `dispatchEvent()`.

- The mount container (the container `mount()` targets, see [Elements](./elements.md)) stands in for `html` and `body` in `composedPath()`.
- A `wheel` listener on the mount container is passive unless its options say
  `passive: false`, as on `body`.
- The `signal` option of `addEventListener()` is unsupported.
- The ref of an unmounted element ignores `addEventListener()` and an
  assignment to a handler property, and `dispatchEvent()` returns `true`.
- An attribute that holds code, such as `onclick="count++"`, runs nothing.
- `document` and `window` have no `body`, `documentElement` or query methods.
- `window` and `document` are imports. `globalThis.window` and
  `globalThis.document` stay undefined.

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

| Events                                                           | bubbles | cancelable | composed |
| ---------------------------------------------------------------- | ------- | ---------- | -------- |
| mouse, pointer, wheel and keyboard events                        | yes     | yes        | yes      |
| `mouseenter`, `mouseleave`, `pointerenter`, `pointerleave`       | no      | no         | no       |
| `scroll`                                                         | no      | no         | no       |
| `focus`, `blur`                                                  | no      | no         | yes      |
| `focusin`, `focusout`, `gotpointercapture`, `lostpointercapture` | yes     | no         | yes      |

- `timeStamp` counts milliseconds since the host started.
- The `view` of a UI event is `null`.
- Removing the focused node, or moving it within the tree, leaves `blur` and
  `focusout` unfired.

## Mouse

Mouse events carry the fields of
[`MouseEvent`](https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent)
from
[Pointer Events](https://www.w3.org/TR/2026/WD-pointerevents4-20261005/#mouse-event-types).
`wheel` carries the fields of
[`WheelEvent`](https://developer.mozilla.org/en-US/docs/Web/API/WheelEvent)
from [Pointer Events](https://www.w3.org/TR/2026/WD-pointerevents4-20261005/#wheel-event-types).

`click`, `auxclick` and `contextmenu` carry the fields of
[`PointerEvent`](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent).
The click of a key has `pointerId` -1, an empty `pointerType` and 0 for every
coordinate.

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
- `isPrimary` is `false`, as in Chromium. WebKit and Firefox report `true`.
- `scrollend` is unsupported.
- A mouse event outside every element has the mount container as `target`,
  where browsers report `body` (Firefox reports `html`). Moving the pointer
  between an element and an empty area gives the mount container as
  `relatedTarget`.

## Pointer

Pointer events carry the fields of
[`PointerEvent`](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent)
and follow
[Pointer Events](https://www.w3.org/TR/2026/WD-pointerevents4-20261005/).
Pointer events come from the mouse only, with `pointerId` 1. `pointercancel` is
never dispatched. The capture methods are in [Elements](./elements.md).

### Host differences

- `tiltX`, `tiltY`, `twist`, `tangentialPressure`, `altitudeAngle`,
  `azimuthAngle`, `persistentDeviceId`, `getCoalescedEvents()` and
  `getPredictedEvents()` of `PointerEvent` are unsupported.
- The pointer boundary events (`pointerout`, `pointerleave`, `pointerover`,
  `pointerenter`) precede the mouse ones, then `pointermove` and `mousemove`
  follow, as in Chromium. WebKit interleaves each pointer event with its mouse
  counterpart.
- After a captured final release the order is `pointerup`, `mouseup`,
  `lostpointercapture`, `click`, then the boundary events, as in Chromium.
  WebKit fires `lostpointercapture` before `mouseup`.
- The boundary events that follow a captured release carry the `button` and
  `which` of the `pointerup`, as in Chromium. WebKit and Firefox report -1 and
  0.

## Keyboard

Keyboard events carry the fields of
[`KeyboardEvent`](https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent)
from [UI Events](https://www.w3.org/TR/2026/WD-uievents-20260221/#events-keyboard-types).
`keydown` and `keyup` go to the focused node, or to the mount container when
nothing is focused. `target` is that node or the mount container, where
browsers report `body`.

| Field         | Value                                           |
| ------------- | ----------------------------------------------- |
| `key`         | `"Unidentified"` when the platform reports none |
| `repeat`      | `false` on `keyup`                              |
| `location`    | 0                                               |
| `isComposing` | `false`                                         |

- `code` is an empty string. `key` identifies the key.
- `keyCode` and `which` give the code of `key` on a US keyboard layout, and 0
  for a key outside it, such as `é`.
- `getModifierState()` accepts Control, Shift, Alt, Meta and Accel. Accel is
  Meta on macOS and Control elsewhere.
- Pressing a modifier alone fires `keydown` and `keyup` with its name as
  `key`.
- The Menu key and Shift+F10 fire `contextmenu` on `keydown`, after the
  `keydown` listeners and unless one canceled the `keydown`, as in Chromium.
  Shift+F10 fires it on Linux and Windows only. It targets the focused element, with `clientX` and `clientY` at the
  center of its box. With nothing focused it targets the element at (1, 1).
  It has `pointerId` 1, `pointerType` `"mouse"`, `button` -1 and no buttons.
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
- After the last node, Tab leaves nothing focused and the next Tab starts at
  the first node, as in Chromium and WebKit. Focus stays in the window.

## Dispatching events

`dispatchEvent()` sends an event that a script builds with an
event constructor.

- `initUIEvent()`, `initMouseEvent()` and `initKeyboardEvent()` are
  unsupported methods.
- The `view` member of an init dictionary is ignored, and `relatedTarget`
  takes any object.
- The `modifierAltGraph`, `modifierCapsLock` and the other `modifier` members
  of an init dictionary are ignored.
- `offsetX` and `offsetY` of a mouse event a script builds equal `clientX` and
  `clientY`, also while it is dispatched.

## Errors in handlers

An error a handler throws, or an `async` handler rejects with, follows Vue's
[error handling](https://vuejs.org/api/composition-api-lifecycle#onerrorcaptured).

The development window reports an error that no hook takes to the terminal
that ran `inca dev`, and shows it as an application error. A packaged app
writes it to stderr.

> [!TIP]
> A packaged app has no readable stderr. Install `app.config.errorHandler` to
> show these errors yourself.
