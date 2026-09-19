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

## App Metadata

> [!NOTE]
> Configuring app metadata through `package.json`'s `"inca"` key is
> provisional. It's expected to move to a dedicated `inca.config.ts`
> in the future.

App metadata for `inca package` comes from the app's own
`package.json`, with an optional `"inca"` key overriding what's derived
from it:

```json [package.json]
{
  "name": "click_counter",
  "version": "1.0.0",
  ...
  "inca": {
    "productName": "Click Counter",
    "identifier": "com.example.click-counter",
    "icon": "assets/icon.icns"
  }
  ...
}
```

- `productName` defaults to `"name"`, with any npm scope (`@org/`)
  stripped.
- `identifier` defaults to a generated `org.inca.<slug>`. It should be
  world-unique, so `inca package` prints a note when it falls back to
  the generated one rather than using it silently.
- `icon` is resolved relative to the app's own directory.
