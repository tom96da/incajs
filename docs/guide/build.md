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
icon, its window, and where a build reads and writes. Every key, with an
example, is listed in [Configuration](../reference/configuration).

## Output

`inca package` writes the packaged app under `outDir` (`dist` by
default), alongside the plain build output:

- macOS — `dist/<productName>.app`. Open it, or run the executable
  inside it directly.
- Linux — `dist/<slug>`, a plain directory named after a slugified
  `productName` (lowercased, non-alphanumeric runs collapsed to `-`),
  holding an executable of the same name.

`inca package` doesn't sign the app yet — macOS Gatekeeper will warn
before it opens.
