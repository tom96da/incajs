---
description: Bundle an Incarnative.js app for production and package it into a distributable application.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Building for Production

`inca build` bundles your app once, with no window opened — the
output lands in `dist`. `inca package` does the same, then wraps the
result into a distributable application: a `.app` on macOS, a plain
directory on Linux.

## Configuration

> [!NOTE]
> Configuring app metadata through `package.json`'s `"inca"` key still
> works, but is deprecated — `inca package` prints a warning, and
> `inca.config.ts` wins when both are present.

When running `inca`, it will automatically try to resolve a config file
named `inca.config.ts` inside your app's root (other JS/JSON extensions
are also supported), for app metadata and where a build reads and
writes:

```ts [inca.config.ts]
import { defineConfig } from "@incajs/cli/config";

export default defineConfig({
  productName: "Click Counter",
  identifier: "com.example.click-counter",
  icon: "assets/icon.icns",
});
```

### `productName`

The app's display name. Defaults to `package.json`'s own `"name"`, with
any npm scope (`@org/`) stripped.

On macOS this is the name the Dock, the menu bar and the Finder show, for
`inca dev` as much as for a packaged app.

### `identifier`

A reverse-DNS-style unique id — e.g. macOS's `CFBundleIdentifier`.
Defaults to a generated `org.inca.<slug>`. It should be world-unique, so
`inca package` prints a note when it falls back to the generated one
rather than using it silently.

### `icon`

The app's icon file, resolved relative to `inca.config.ts`'s own
directory. On macOS it is what the Dock shows, for `inca dev` as much as
for a packaged app.

### `version`

The packaged app's version. Defaults to `package.json`'s own
`"version"`.

### `entry`

The app's entry point, overriding the automatic entry resolution
(`src/main.mts`, or `src/App.vue`).

### `outDir`

Where a build's output is written. Defaults to `dist`.
