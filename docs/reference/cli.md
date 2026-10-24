---
description: Reference of the inca CLI commands and their options.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Command Line Interface

Every command reports a failure as `[inca] <command> failed (<code>): …`.
See [Error Codes](./errors) for what each code means.

| Command                 | Description |
| ----------------------- | ----------- |
| `inca --help`           | Lists the commands. |
| `inca <command> --help` | Lists the options of the command. |
| `inca --version`        | Prints the version of `@incajs/cli`. |

An unknown command prints the command list and exits with code 1.

## `inca dev`

Watches your app and opens a live-reloading development window, reloading
on every change. It exits with code 0 when you close the window and
non-zero if the host crashes.

When the config or the entry cannot be used before the first build, it
prints the error and retries when you save the config or the entry. It
stops before building when no host binary can be found.

When no `entry` is set, it selects the entry again after a `src/main` file or
`src/App.vue` is created or deleted, and reloads the development window if the
selection changed. While no entry exists, it prints the error and keeps the
window open until one appears.

### Usage

```sh
$ inca dev
```

### Options

| Flag                 | Description |
| -------------------- | ----------- |
| `--experimental-hmr` | Reloads only the changed module instead of the whole bundle. |

> [!WARNING]
> `--experimental-hmr` is experimental. See the [HMR guide](../guide/hmr) for
> what it does differently.

## `inca build`

Bundles your app for production, once, with no window opened. See
[Building for Production](../guide/build).

### Usage

```sh
$ inca build
```

### Options

| Flag             | Description |
| ---------------- | ----------- |
| `--sourcemap`    | Writes `bundle.js.map` beside the bundle. |
| `--no-sourcemap` | Turns the map off when the [`sourcemap`](./configuration#sourcemap) key turns it on. |

When both flags are given, `--no-sourcemap` wins.

## `inca package`

Builds your app, then packages it with a prebuilt host binary into a
distributable application: a `.app` on macOS, a plain directory on
Linux. See [Building for Production](../guide/build).

### Usage

```sh
$ inca package
```

### Options

| Flag             | Description |
| ---------------- | ----------- |
| `--sourcemap`    | Writes `bundle.js.map` beside the bundle. |
| `--no-sourcemap` | Turns the map off when the [`sourcemap`](./configuration#sourcemap) key turns it on. |

When both flags are given, `--no-sourcemap` wins.

## `inca config`

Prints the settings the host applies to your app as JSON: its name, its
identifier and its window. A `null` marks a value left to the host: the
root element sizes the window, and an unset limit stays open. It reads the
config as `inca build` does, builds nothing and opens no window.

A setting whose value the host cannot use fails with
[`ERR_INCA_CONFIG_INVALID`](./errors#err-inca-config-invalid), naming the
setting, its value and the kind of value it must be. The other commands
don't check the values, so the host opens with the defaults.

### Usage

```sh
$ inca config
```

## Environment Variables

| Variable        | Description          |
| --------------- | -------------------- |
| `INCA_HOST_BIN` | Path to a specific host binary, used by `dev`, `package` and `config` in place of automatic resolution, e.g. on a custom or unpublished build. |
| `INCA_EXPERIMENTAL_HMR` | Same as `--experimental-hmr`, for `inca dev`. See the [HMR guide](../guide/hmr). |
