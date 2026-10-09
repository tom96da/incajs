---
description: Every style key Incarnative.js renders, how elements and `class` work, and which Vue features this renderer doesn't support yet.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Elements & Styles

> [!TIP]
> This page describes today's surface. Closing the gap with Vue and DOM web
> standards is an ongoing direction for the project.

## Elements are just boxes

Every tag other than `"text"` — `<div>`, `<span>`, `<button>`, anything —
renders as a generic styled container. Elements carry no default styles, so
a `<button>` starts as the same plain box as a `<div>` and `:style` sets
everything. Only `"text"` renders differently: its `value` attribute followed
by its descendants' text in child order. Descendants are not rendered as
separate elements.

### `button`

A `button` is a box that also takes keyboard focus and clicks from the
keyboard.

- Its `tabindex` is 0 by default, so it takes focus on its own. An explicit
  `tabindex` overrides that, and `el.tabIndex` reads `0` until a value is
  assigned.
- `Enter`, `Space` and the `disabled` attribute are described under
  [`click`](./events#focus).

```vue
<template>
  <button
    :style="{ width: 80, height: 32, background: 0x2266ff }"
    :disabled="saving"
    @click="save"
  >
    Save
  </button>
</template>
```

## Recognized style keys <Badge type="warning" text="unreleased" />

`:style` takes an object. Only the keys below are read, each with its own
accepted value shapes. A value of the wrong shape sets the property to its
default, even when the key held a valid value before. An unrecognized key,
such as a misspelled one, has no effect.

The host prints one warning line on stderr each time a style sets an
unrecognized key or an invalid color.

Removing a key from the object, or setting it to `null` or `undefined`,
restores that property to its default. Setting `style` to a string or `null`
clears every key set before.

| Key | Value | Notes |
| --- | --- | --- |
| `display` | `"flex"` \| `"block"` \| `"grid"` \| `"none"` | |
| `flexDirection` | `"row"` \| `"column"` \| `"row_reverse"` \| `"column_reverse"` | |
| `justifyContent` | `"start"` \| `"end"` \| `"center"` \| `"stretch"` | main-axis alignment |
| `alignItems` | `"start"` \| `"end"` \| `"center"` \| `"stretch"` | cross-axis alignment |
| `gap` | `number` (px) | applied on both axes |
| `width` | `number` (px) \| `"auto"` | |
| `height` | `number` (px) \| `"auto"` | |
| `borderWidth` | `number` (px) | applied to all four sides |
| `background` | color (below) | fill |
| `borderColor` | color (below) | |
| `cornerRadius` | `number` (px) | applied to all four corners |
| `textColor` | color (below) | cascades to descendant text leaves |
| `textSize` | `number` (px) | cascades to descendant text leaves |
| `overflow` / `overflowX` / `overflowY` | `"visible"` \| `"hidden"` \| `"scroll"` \| `"auto"` | `"hidden"` clips, `"scroll"` (and its alias `"auto"`) scrolls with the wheel; `overflowX`/`overflowY` win over `overflow` |
| `padding` / `paddingX` / `paddingY` / `paddingTop` / `paddingRight` / `paddingBottom` / `paddingLeft` | `number` (px, `>= 0`) | a negative, `NaN` or infinite value is ignored |
| `margin` / `marginX` / `marginY` / `marginTop` / `marginRight` / `marginBottom` / `marginLeft` | `number` (px, negative allowed) \| `"auto"` | |
| `flexGrow` / `flexShrink` | `number` (`>= 0`) | |
| `opacity` | `number` (`0` to `1`) | clamped; applies to descendants too |
| `minWidth` / `minHeight` / `maxWidth` / `maxHeight` | `number` (px) \| `"auto"` | `"auto"` is the default limit: none for the `max` keys, the content size for a flex item's `min` keys |

For `padding` and `margin`, a side key beats its axis key (`paddingX` for left
and right, `paddingY` for top and bottom), and an axis key beats the
shorthand. A value of the wrong shape counts as unset and the next key
applies.

Scrollbars are not supported. `stopPropagation()` and `preventDefault()` on
a scrolling container's [`wheel`](./events#mouse) event act on the scroll
as described there.

`position` and `zIndex` are not supported, and their keys are ignored.

### Color format

`background`, `borderColor`, and `textColor` each accept:

- a numeric `0xRRGGBB` literal, e.g. `0xff0000`
- a `"#rrggbb"` or `"#rgb"` string, e.g. `"#ff0000"` or `"#f00"`

A number must lie from `0` to `0xffffff`. A string must be `#` followed by 3 or
6 hex digits. Any other number or string sets the property to its default and
prints a warning. Alpha channels and color names such as `"red"` are not
supported.

## `class`, and `style` as a string, do nothing

`class="..."` is a valid Vue binding — the compiler accepts it and the
renderer stores it as a plain attribute — but nothing reads it to draw
anything.

`style` is different: Vue's compiler routes a static `style="color: red"`
attribute and a bound `:style="someString"` to the same `style` prop, and
`patchProp`'s `patchStyle` only accepts that prop as an object — a string
value is ignored either way. So a plain `style="..."` attribute can
*never* work, since it's always a string; only `:style` bound to an
object, with the keys above, has any visible effect.

```vue
<template>
  <!-- does nothing visually: class, and style as a string -->
  <div class="card" style="color: red">…</div>
  <div :style="'color: red'">…</div>
  <!-- works -->
  <div :style="{ textColor: 0xff0000 }">…</div>
</template>
```

## Vue features that don't work yet

A few common Vue features depend on DOM concepts this renderer doesn't
have, and using them fails the build:

- **`v-show`**, **`v-model`** on a native element, and **`<Transition>`**
  all fail to build with a Rollup `MISSING_EXPORT` error (`"vShow"`/
  `"vModelText"`/`"Transition"` is not exported by the runtime).
  `v-model` on a non-`<input>`/`<textarea>`/`<select>` element (which is
  every inca element, since there's no native input yet) additionally
  fails at the template-compiler stage with its own error, independent of
  the missing export.
- **No timers or `fetch`**: `setTimeout`/`setInterval`/`fetch` aren't
  provided as globals by the JS runtime Incarnative.js embeds.
- **No text input**: an editable text element is still unimplemented (see
  the [Roadmap](../roadmap)) — not permanently unsupported, just not built
  yet.
- **`ref` only exposes `.focus()`/`.blur()`** on the underlying element —
  see [Events: Focus](./events#focus) for how focus works.

Event modifiers such as `.stop`, `.once` and `@keydown.enter` are documented
in [Events: Event modifiers](./events#event-modifiers).

## Bootstrapping: `src/main.ts`

Incarnative.js wraps `src/App.vue` in a synthesized entry equivalent to the
one below. Write `src/main.ts` yourself for full control over bootstrapping.
The [`entry`](./configuration#entry) setting lists the files Incarnative.js
looks for.

```ts [src/main.ts]
import { createIncaApp } from "incajs/vue";
import App from "./App.vue";

createIncaApp(App).mount();
```

`createIncaApp(rootComponent, rootProps?)` creates a Vue `App` whose
`mount()` targets the host's root container when called with no
argument; otherwise it behaves exactly like Vue's own
`App` (`use`, `mixin`, `component`, `directive`, `unmount`, ...).
