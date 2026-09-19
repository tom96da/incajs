<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# @incajs/cli

The `inca` command for building and running [Incarnative.js](https://github.com/tom96da/incajs)
apps: `inca dev`, `inca build`, and `inca package`.

## Install

```sh
npm i -D @incajs/cli
```

## App entry

No entry point is required. Drop a `src/App.vue` and that's a whole
app. Commit a `src/main.mts` instead for full control over
bootstrapping; it wins outright when both exist.

## Commands

- **`inca dev`** — watches your app and opens a live-reloading window,
  reloading on every change.
- **`inca build`** — bundles your app for production, once, with no
  window opened.
- **`inca package`** — builds your app, then packages it with a
  prebuilt `inca-host` into a distributable application: a `.app` on
  macOS, a plain directory on Linux (other platforms aren't supported
  yet).

When running `inca`, it will automatically try to resolve a config file
named `inca.config.ts` inside your app's root (other JS/JSON extensions
are also supported), for app metadata and build settings:

```ts [inca.config.ts]
import { defineConfig } from "@incajs/cli/config";

export default defineConfig({
  // the app's display name
  productName: "Click Counter",
  // macOS's CFBundleIdentifier or equivalent
  identifier: "com.example.click-counter",
  // the packaged app's icon file
  icon: "assets/icon.icns",
});
```

An `identifier` should be world-unique, so `inca package` prints a note
when it falls back to the generated one rather than using it silently.

## Requirements

`inca`'s host binary is pulled in automatically as a platform-specific
optional dependency (Linux and macOS on x64/arm64; Windows isn't
supported yet) — neither Cargo nor Rust are needed. Set `INCA_HOST_BIN`
to use a specific binary instead, e.g. on a platform with no published
one yet, or a custom build.

## How it works

`inca dev`/`build`/`package` bundle your app with Vite, then (for `dev`
and `package`) launch and supervise `inca-host`, the native runtime
that renders it, as a child process.
