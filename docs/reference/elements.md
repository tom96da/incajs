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

Every tag other than `"input"` (`<div>`, `<span>`, `<button>`, anything)
renders as a generic styled container. Elements carry no default styles, so a
`<button>` starts as the same plain box as a `<div>` and `:style` sets
everything. `"input"` is a text field, described below.

### `button`

A `button` is a box that also takes keyboard focus.

- Its `tabindex` is 0 by default, so it takes focus on its own. An explicit
  `tabindex` overrides that.
- `Enter` and `Space` click it from the keyboard.

```vue
<template>
  <button
    :style="{ width: 80, height: 32, backgroundColor: '#2266ff' }"
    :disabled="saving"
    @click="save"
  >
    Save
  </button>
</template>
```

### `input`

An `input` is a one-line text field.

- Every `type` renders as a text field, so `type="password"` shows the
  characters.
- It has no default size. Without a height in `:style` it is one line of its
  `fontSize` tall, and its width comes from `:style`. The text line sits at
  the top of the box.
- The caret is a 1px bar in the text color and does not blink. The selection
  is a fixed light blue, and the `placeholder` shows in the text color at half
  opacity.
- Word-wise moves and deletes, Cmd+Left, Cmd+Right, Cmd+Backspace, Ctrl+A,
  Ctrl+E and Option with an arrow on macOS, undo, and a double-click word
  selection are unsupported.
- `selectionStart`, `selectionEnd`, `selectionDirection`,
  `setSelectionRange()`, `select()`, `defaultValue`, `maxLength` and `size`
  are missing.

### `style`

A `style` element applies its rules to every element of the app and is never
drawn. The elements of a `<style>` block and of an imported stylesheet are
children of the mount container. You can also render one with `h("style", css)`
or `<component is="style">`.

- The declarations take the keys and values of `:style`. The host drops any
  other declaration, see [Why does a rule of my stylesheet have no
  effect?](../guide/troubleshooting#why-does-a-rule-of-my-stylesheet-have-no-effect).
- `!important` is read as a normal declaration, so an inline value still wins.
- `@media` and the other at-rules, including an `@import` that the build did not
  resolve, have no effect.
- Native CSS nesting, custom properties, `var()` and `url()` values have no
  effect.
- `v-bind()` in a `<style>` block fails the build.
- The `media`, `type` and `disabled` attributes are ignored.
- The host draws no focus indicator and no look for `:hover` or `:active`.
- `:hover` and `:active` change only while the app has an event listener, a
  focusable element or a rule that names them.
- `:focus-visible` and the other interactive pseudo-classes never match.

## Recognized style keys

`:style` takes an object. Only the keys below are read, each with its own
accepted value shapes. A value of the wrong shape sets the property to its
default, even when the key held a valid value before. An unrecognized key,
such as a misspelled one, has no effect.

The host prints a warning line on stderr the first time an element sets an
unrecognized key or a value of a shape the host does not support. An empty
string `''` on a known key prints no warning.

Removing a key from the object, or setting it to `null` or `undefined`,
restores that property to its default. Setting `style` to a string or `null`
clears every key set before.

A px size is a number or a string such as `"10px"`. A string that reads as
zero, such as `"0"` or `"0.0"`, also works. Other units are not supported.
`opacity`, `flexGrow` and `flexShrink` also take a decimal string such as
`"0.5"`, without `%`. A key that starts with `-` is unknown.

| Key | Value | Notes |
| --- | --- | --- |
| `display` | `"flex"` \| `"block"` \| `"grid"` \| `"none"` | |
| `flexDirection` | `"row"` \| `"column"` \| `"row-reverse"` \| `"column-reverse"` | |
| `justifyContent` | `"start"` \| `"end"` \| `"flex-start"` \| `"flex-end"` \| `"center"` \| `"stretch"` | main-axis alignment |
| `alignItems` | `"start"` \| `"end"` \| `"flex-start"` \| `"flex-end"` \| `"center"` \| `"stretch"` | cross-axis alignment |
| `gap` | px size (`>= 0`) | applied on both axes |
| `width` | px size (`>= 0`) \| `"auto"` | |
| `height` | px size (`>= 0`) \| `"auto"` | |
| `borderWidth` | px size (`>= 0`) | applied to all four sides |
| `backgroundColor` | color (below) | fill |
| `borderColor` | color (below) | |
| `borderRadius` | px size (`>= 0`) | applied to all four corners |
| `color` | color (below) | cascades to descendant text leaves |
| `fontSize` | px size (`>= 0`) | cascades to descendant text leaves |
| `overflow` / `overflowX` / `overflowY` | `"visible"` \| `"hidden"` \| `"clip"` \| `"scroll"` \| `"auto"`; `overflow` also takes two of them, x then y | `"hidden"` clips and script can scroll it; `"scroll"` (and its alias `"auto"`) also scrolls with the wheel; `overflowX`/`overflowY` win over `overflow` |
| `padding` / `paddingX` / `paddingY` / `paddingTop` / `paddingRight` / `paddingBottom` / `paddingLeft` | px size (`>= 0`) | a negative, `NaN` or infinite value is ignored |
| `margin` / `marginX` / `marginY` / `marginTop` / `marginRight` / `marginBottom` / `marginLeft` | px size (negative allowed) \| `"auto"` | |
| `flexGrow` / `flexShrink` | number or decimal string (`>= 0`) | |
| `opacity` | number or decimal string (`0` to `1`) | clamped; applies to descendants too |
| `minWidth` / `minHeight` / `maxWidth` / `maxHeight` | px size (`>= 0`) \| `"auto"` | `"auto"` is the default limit: none for the `max` keys, the content size for a flex item's `min` keys |

Content taller than the window is cut off at the window edge; see
[Why doesn't my content scroll](../guide/troubleshooting#why-doesn-t-my-content-scroll-when-it-is-taller-than-the-window).

For `padding` and `margin`, a side key beats its axis key (`paddingX` for left
and right, `paddingY` for top and bottom), and an axis key beats the
shorthand. A value of the wrong shape counts as unset and the next key
applies.

Scrollbars are not supported.

`position` and `zIndex` are not supported, and their keys are ignored.

### Color format

`backgroundColor`, `borderColor` and `color` each take a numeric `0xRRGGBB`
literal, e.g. `0xff0000`, or a string of the CSS
[`<color>`](https://developer.mozilla.org/en-US/docs/Web/CSS/color_value) type.

A number must lie from `0` to `0xffffff`. Other notations, such as color names,
`hsl()` and `currentcolor`, are not supported. `rgb()` does not take `none`,
`calc()` or `var()`, and percentage channels need whitespace between them.

## `style` as a string does nothing

Vue's compiler routes a static `style="color: red"`
attribute and a bound `:style="someString"` to the same `style` prop, and
`patchProp`'s `patchStyle` only accepts that prop as an object. A string
value is ignored either way. So a plain `style="..."` attribute can
*never* work, since it's always a string; only `:style` bound to an
object, with the keys above, has any visible effect.

```vue
<template>
  <!-- does nothing visually: style as a string -->
  <div style="color: red">…</div>
  <div :style="'color: red'">…</div>
  <!-- works -->
  <div :style="{ color: '#ff0000' }">…</div>
</template>
```

## Template refs

A template `ref` is an element handle. Its members behave as described for
[`Element`](https://developer.mozilla.org/en-US/docs/Web/API/Element) and
[`HTMLElement`](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement),
with the differences below. The listener methods are described in
[Event targets](./events#event-targets).

- A box read returns the box of the last drawn frame, so `onMounted` reads
  zeros.
- `offsetParent` is the parent element, or `null` when the node or its parent
  has no box.
- A scroll write is stored at once. A read clamps it to the maximum of the
  last drawn frame, so a write beyond that maximum, or one made before the
  first draw, reads smaller until the next frame.
- `scrollWidth` and `scrollHeight` of a `"visible"` or `"clip"` element equal
  its client size.
- The scroll range of a `"hidden"` or `"scroll"` element leaves out the end
  margins of its children and content that overflows a child.
- Every scroll applies at once, whatever `behavior` says.
- `focus()` and `scrollIntoView()` on a node that has no box yet scroll when
  the next frame draws it.
- Only `pointerId` 1 is accepted by `setPointerCapture()`,
  `releasePointerCapture()` and `hasPointerCapture()`.
- `parent` is the parent element, or the fragment that holds the node.
- The `parentNode` of the mount container is `document`.
- A node that a template removes (for example with `v-if`) drops its listeners
  for good. Appending it again does not restore them. `removeChild()` and
  `replaceChild()` keep the listeners of the node they remove.
- `document.createDocumentFragment()` returns a fragment without these members:
  - `querySelector()` and `querySelectorAll()`
  - `children`
  - `getElementById()`
  - the listener methods

  The global `DocumentFragment` does not exist.
- The methods below store U+FFFD for each half of a surrogate pair they split:
  - `splitText()`
  - `substringData()`, `insertData()`, `deleteData()`, `replaceData()`
- The objects these members return are not instances of the standard classes,
  so `instanceof` against the class name is `false`:
  - `children` (`HTMLCollection`)
  - `childNodes` (`NodeList`)
  - `attributes` (`NamedNodeMap`)
  - the items of `attributes` (`Attr`)
  - `querySelectorAll()` (`NodeList`)
- The pseudo-class rules below hold for these methods:
  - `closest()`
  - `matches()`
  - `querySelector()`
  - `querySelectorAll()`

  They accept `:focus-visible` and the other interactive and form-state
  pseudo-classes and never match them, so `:not(:focus-visible)` matches
  every element. `:lang()` and `:state()` never match. `:dir(ltr)` matches every
  element and any other `:dir()` value never matches. `:disabled` and
  `:enabled` apply to `button` and `input` only.
- These members of `attributes` are missing:
  - `setNamedItem()` and `removeNamedItem()`
  - `getNamedItemNS()`, `setNamedItemNS()` and `removeNamedItemNS()`
- An `Attr` has no `Node` members.

## Vue features that don't work yet

A few common Vue features depend on DOM concepts this renderer doesn't
have, and using them fails the build:

- **`v-show`** and **`<Transition>`** fail to build with a Rollup
  `MISSING_EXPORT` error (`"vShow"`/`"Transition"` is not exported by the
  runtime). So does **`v-model`** on a `<select>`, a checkbox, a radio button
  or an `<input>` with a bound `:type` (`"vModelSelect"`, `"vModelCheckbox"`,
  `"vModelRadio"`, `"vModelDynamic"`). `v-model` on a `<textarea>` builds,
  and the `<textarea>` stays a plain box. On any other element `v-model`
  fails at the template-compiler stage.
- **No `fetch`**: `fetch` isn't provided as a global by the JS runtime
  Incarnative.js embeds.
- **`ref` lacks other element members**: `classList`, `className`, `dataset`
  and `style` are missing.

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
`mount()` targets the host's mount container when called with no
argument; otherwise it behaves exactly like Vue's own
`App` (`use`, `mixin`, `component`, `directive`, `unmount`, ...).
