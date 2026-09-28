---
description: The window an inca app opens in — its size, its title, its menu, and how it closes.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Application Window

An app opens in one window. Everything around its content — the size it
starts at, the title it carries, its menu — is set under `window` in
`inca.config.ts` (see [Configuration](../reference/configuration#the-window)
for the full key list and an example), or falls back to a default.

These apply to the development window and to a packaged app alike.

> [!NOTE]
> `inca dev` reads `inca.config.ts` once, at startup. Reloading it on
> change isn't supported yet — restart `inca dev` to pick up an edit.

## Size

The window opens at `window.width` × `window.height`.

With neither set, it takes the `width` and `height` your app's root element
declares, so a fixed-size app gets a window that fits it:

```vue [src/App.vue]
<template>
  <div :style="{ width: 400, height: 300 }">…</div>
</template>
```

Each dimension falls through on its own — a config that sets only `width`
still takes its height from the root element. With neither source giving
one, the window opens at 800 × 600.

`window.minWidth` and `window.minHeight` set the smallest the window can
be. A window opens at least that size, and a dimension you leave out is
unconstrained.

## Resizing

A window holds the size it opened at unless `window.resizable` is `true`.

An element sizes in pixels or to its content, with no way to fill the
window, so a window grown past the app's root element shows empty space
around it. Let a user resize one whose layout survives it:

```ts [inca.config.ts]
window: { width: 400, height: 300, resizable: true },  // [!code ++]
```

## Title

The window's title bar carries `window.title`, or
[`productName`](../reference/configuration#productname) where that is
unset.

## The application menu

Every app gets an application menu with a `Quit` item, on `⌘Q` on macOS and
`Ctrl+Q` on Linux. macOS is the only platform that draws a menu bar for it
today. There is no way to add items to it yet.

## Under development

The development window, and `--experimental-hmr` on top of it, add a few
dev-only quirks on top of the behavior above.

Under `--experimental-hmr`, the window's first launch can briefly open at
the wrong size before snapping to the correct one — see the
[HMR guide](./hmr#known-limitations).

In the development window, resizing it by hand keeps that size across
later reloads; a non-resizable window snaps back to its configured size on
each reload.

Closing the development window also stops the dev process.

## Closing the app

Closing the window quits the app.
