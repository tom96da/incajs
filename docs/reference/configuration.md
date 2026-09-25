---
description: Every key inca.config.ts takes, its type, and what it defaults to.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Configuration

`inca` reads `inca.config.ts` from your app's root — other JS and JSON
extensions work too. `defineConfig` is a typing helper; it returns what you
give it.

```ts [inca.config.ts]
import { defineConfig } from "@incajs/cli/config";

export default defineConfig({
  productName: "Click Counter",
  identifier: "com.example.click-counter",
  icon: "assets/icon.icns",
  window: {
    width: 1024,
    height: 768,
  },
});
```

> [!NOTE]
> `package.json`'s `"inca"` key still works and is deprecated. `inca
> package` prints a warning, and `inca.config.ts` wins where both set the
> same key.

## The app

### `productName`

- Type: `string`
- Default: `package.json`'s own `"name"`, with any npm scope (`@org/`)
  stripped

The name the operating system shows for the running app — the Dock and the
menu bar on macOS, the taskbar on Linux — under `inca dev` as much as once
packaged. It also names the packaged app's own directory, so it can't hold
`/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|` or a null byte, can't be `.` or
`..`, and can't open or close with a space. See
[`ERR_INCA_PRODUCT_NAME_INVALID`](./errors#err-inca-product-name-invalid).

### `identifier`

- Type: `string`
- Default: a generated `org.inca.<slug>`

A reverse-DNS-style unique id: `CFBundleIdentifier` on macOS, the window's
`app_id` on Wayland and its `WM_CLASS` on X11. It should be world-unique,
so `inca package` prints a note when it falls back to the generated one.

### `icon`

- Type: `string` — a path, resolved from `inca.config.ts`'s own directory
- Default: none

What the Dock shows, under `inca dev` as much as once packaged.

### `version`

- Type: `string`
- Default: `package.json`'s own `"version"`

The packaged app's version.

## The window

Every key here is covered in prose by
[Application Window](../guide/window).

### `window.width` / `window.height`

- Type: `number` — pixels
- Default: the `width`/`height` the app's root element declares, then
  800 × 600

The size the window opens at. Each dimension falls through on its own.

### `window.title`

- Type: `string`
- Default: `productName`

The window's title bar.

### `window.resizable`

- Type: `boolean`
- Default: `false`

Whether a user can resize the window.

### `window.minWidth` / `window.minHeight`

- Type: `number` — pixels
- Default: none

The smallest the window can be. It opens at least this size.

## The build

### `entry`

- Type: `string` — a path
- Default: a committed `src/main.mts`, or `src/App.vue` wrapped in a
  synthesized entry

The app's entry point.

### `outDir`

- Type: `string` — a path
- Default: `"dist"`

Where a build's output is written.
