---
description: The window an inca app opens in — its size, its title, its menu, and how it closes.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Application Window <Badge type="warning" text="unreleased" />

An app opens in one window. Everything around its content — the size it
starts at, the title it carries, its menu — is set in `inca.config.ts`, or
falls back to a default.

```ts [inca.config.ts]
import { defineConfig } from "@incajs/cli/config";

export default defineConfig({
  productName: "Click Counter",
  window: {
    width: 1024,
    height: 768,
    title: "Click Counter",
  },
});
```

These apply to `inca dev` and to a packaged app alike. `inca dev` reads the
config once, at startup — restart it after editing `inca.config.ts`.

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
window: { width: 400, height: 300, resizable: true },
```

## Title

The window's title bar carries `window.title`, or
[`productName`](../reference/configuration#productname) where that is
unset.

## The application menu

Every app gets an application menu with a `Quit` item, on `⌘Q` on macOS and
`Ctrl+Q` on Linux. macOS is the only platform that draws a menu bar for it
today. There is no way to add items to it yet.

## Closing the app

Closing the window quits the app. Under `inca dev`, the dev process stops
with it.
