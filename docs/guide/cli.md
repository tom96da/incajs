---
description: Reference of the inca CLI commands — dev, build, and package.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Command Line Interface

## `inca dev`

Watches your app and opens a live-reloading window, reloading on every
change.

### Usage

```sh
inca dev
```

## `inca build`

Bundles your app for production, once, with no window opened. See
[Building for Production](./build).

### Usage

```sh
inca build
```

## `inca package`

Builds your app, then packages it with a prebuilt `inca-host` into a
distributable application: a `.app` on macOS, a plain directory on
Linux. See [Building for Production](./build).

### Usage

```sh
inca package
```

## Environment Variables

| Variable        | Description          |
| --------------- | -------------------- |
| `INCA_HOST_BIN` | Path to a specific `inca-host` binary, used by `dev` and `package` in place of automatic resolution — e.g. on a custom or unpublished build. |
