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

`inca` reads `inca.config.ts` from your app's root, for the app's name, its
icon, its window, and where a build reads and writes:

```ts [inca.config.ts]
import { defineConfig } from "@incajs/cli/config";

export default defineConfig({
  productName: "Click Counter",
  identifier: "com.example.click-counter",
  icon: "assets/icon.icns",
});
```

Every key is listed in [Configuration](../reference/configuration).
