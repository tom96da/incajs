---
description: Every style key Incarnative.js renders, how elements and `class` work, and which Vue features this renderer doesn't support yet.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Elements & Styles

> [!TIP]
> This page describes today's surface — the style keys, boxed elements,
> and missing Vue features below aren't a permanent ceiling. Closing the
> gap with Vue/DOM web standards is an active, ongoing direction for the
> project.

## Elements are just boxes

Every tag other than `"text"` — `<div>`, `<span>`, `<button>`, anything —
renders identically, as a generic styled container. The tag name itself
has no rendering meaning; only `"text"` is special, and only because it
renders text content: its `value` attribute followed by its descendants'
text in child order. Descendants are not rendered as separate elements.

```vue
<template>
  <button :style="{ background: 0x2266ff }">Hi</button>
</template>
```

`<button>` here isn't a native button — it's a `<div>`-shaped box with a
background color, and nothing makes it clickable on its own (bind
`@click`, same as any other element).

## Recognized style keys

`:style` takes an object; only these keys are read, each with its own
accepted value shapes. Anything else — a wrong shape, an unrecognized
enum string, a misspelled key — is silently ignored, not an error. A key
that was set before and then gets a wrong shape reverts to its default.

Removing a key from the object, or setting it to `null` or `undefined`,
restores that property to its default. Setting `style` to a string or `null`
clears every key set before.

| Key | Value | Notes |
| --- | --- | --- |
| `display` | `"flex"` \| `"block"` \| `"grid"` \| `"none"` | |
| `flex_direction` | `"row"` \| `"column"` \| `"row_reverse"` \| `"column_reverse"` | |
| `justify_content` | `"start"` \| `"end"` \| `"center"` \| `"stretch"` | main-axis alignment |
| `align_items` | `"start"` \| `"end"` \| `"center"` \| `"stretch"` | cross-axis alignment |
| `gap` | `number` (px) | applied on both axes |
| `width` | `number` (px) \| `"auto"` | |
| `height` | `number` (px) \| `"auto"` | |
| `border_width` | `number` (px) | applied to all four sides |
| `background` | color (below) | fill |
| `border_color` | color (below) | |
| `corner_radius` | `number` (px) | applied to all four corners |
| `text_color` | color (below) | cascades to descendant text leaves |
| `text_size` | `number` (px) | cascades to descendant text leaves |
| `overflow` / `overflow_x` / `overflow_y` | `"visible"` \| `"hidden"` \| `"scroll"` \| `"auto"` | `"hidden"` clips, `"scroll"` (and its alias `"auto"`) scrolls with the wheel; `overflow_x`/`overflow_y` win over `overflow` |
| `padding` / `padding_x` / `padding_y` / `padding_top` / `padding_right` / `padding_bottom` / `padding_left` | `number` (px, `>= 0`) | a negative, `NaN` or infinite value is ignored |
| `margin` / `margin_x` / `margin_y` / `margin_top` / `margin_right` / `margin_bottom` / `margin_left` | `number` (px, negative allowed) \| `"auto"` | |
| `flex_grow` / `flex_shrink` | `number` (`>= 0`) | |
| `opacity` | `number` (`0` to `1`) | clamped; applies to descendants too |
| `min_width` / `min_height` / `max_width` / `max_height` | `number` (px) \| `"auto"` | `"auto"` is the default limit: none for `max_*`, the content size for a flex item's `min_*` |

For `padding` and `margin`, a side key beats its axis key (`_x` for left and
right, `_y` for top and bottom), and an axis key beats the shorthand. A value
of the wrong shape counts as unset and the next key applies.

Scrollbars are not supported. `stopPropagation()` and `preventDefault()` on
a scrolling container's [`wheel`](./events.md#wheel) event act on the scroll
as described there.

`position` and `z_index` are not supported, and their keys are ignored.

Keys are snake_case, not camelCase — `flexDirection`, `justifyContent`,
etc. are unrecognized keys and are ignored the same as any other typo.

### Color format

`background`, `border_color`, and `text_color` each accept:

- a numeric `0xRRGGBB` literal, e.g. `0xff0000`
- a `"#rrggbb"` or `"#rgb"` string, e.g. `"#ff0000"` or `"#f00"`

There's no alpha channel and no named colors (`"red"` doesn't parse). A
string missing the `#`, the wrong digit count, or non-hex characters is
ignored like any other malformed value.

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
  <div :style="{ text_color: 0xff0000 }">…</div>
</template>
```

## Vue features that don't work yet

`.vue` files compile against `@vue/runtime-core` directly (not the `vue`
meta-package, which pulls in `@vue/runtime-dom`'s browser-specific
pieces). A few common Vue features live only in `runtime-dom` or depend
on DOM concepts this renderer doesn't have, and using them fails the
build rather than silently no-oping:

- **`v-show`**, **`v-model`** on a native element, and **`<Transition>`**
  all fail to build with a Rollup `MISSING_EXPORT` error (`"vShow"`/
  `"vModelText"`/`"Transition"` is not exported by `@vue/runtime-core`).
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

Event modifiers (`.once`/`.passive`/`.capture`) are documented in
[Events: Event modifiers](./events#event-modifiers).

## Bootstrapping: `src/main.mts`

Incarnative.js looks for `src/App.vue` and wraps it in a synthesized entry
equivalent to this — write `src/main.mts` yourself for full control over
bootstrapping (it wins outright when both files exist). The file is
`src/main.mts`, `src/main.ts` or `src/main.js`, and the first one that
exists in that order is used:

```ts [src/main.mts]
import { createIncaApp } from "incajs/vue";
import App from "./App.vue";

createIncaApp(App).mount();
```

`createIncaApp(rootComponent, rootProps?)` creates a Vue `App` whose
`mount()` targets the host's root container when called with no
argument; otherwise it behaves exactly like `@vue/runtime-core`'s own
`App` (`use`, `mixin`, `component`, `directive`, `unmount`, ...).
