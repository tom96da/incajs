---
description: Reference of the inca CLI commands — dev, build, and package.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Command Line Interface

Every command reports a failure as `[inca] <command> failed (<code>): …` —
see [Error Codes](../reference/errors) for what each one means.

## `inca dev`

Watches your app and opens a live-reloading development window, reloading
on every change.

### Usage

```sh
inca dev
```

### Options

| Flag                 | Description |
| -------------------- | ----------- |
| `--experimental-hmr` | Reloads only the changed module instead of the whole bundle. |

> [!WARNING]
> `--experimental-hmr` is experimental. See the [HMR guide](./hmr) for
> what it does differently and its known limitations.

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
| `INCA_EXPERIMENTAL_HMR` | Same as `--experimental-hmr`, for `inca dev`. See the [HMR guide](./hmr). |
