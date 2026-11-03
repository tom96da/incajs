---
description: Bundle an Incarnative.js app for production and package it into a distributable application.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Building for Production

`inca build` bundles your app once, with no window opened. The
output lands in `dist`. `inca package` does the same, then wraps the
result into a distributable application: a `.app` on macOS, a plain
directory on Linux.

## Configuration

Configuration is read from `inca.config.ts` in your app's root: the app's
name, its icon, its window, and where a build reads and writes. Every key, with an
example, is listed in [Configuration](../reference/configuration).

## Output

A build writes `bundle.js` and its chunks to `outDir`. With the
[`sourcemap`](../reference/configuration#sourcemap) key or the `--sourcemap`
flag it also writes `bundle.js.map`, and the host reads the map to name the
`.vue` or `.ts` file, line and column in an error. A build copies the files of
`public/` into `outDir`, and `inca package` copies them into the packaged app.

`inca package` writes the packaged app under `outDir` (`dist` by
default), alongside the plain build output:

- macOS: `dist/<productName>.app`. Open it, or run the executable
  inside it directly.
- Linux: `dist/<slug>`, a plain directory named after a slugified
  `productName` (lowercased, non-alphanumeric runs collapsed to `-`),
  holding an executable of the same name.

`inca package` also copies the license files of the runtime into the app, unless
`INCA_HOST_BIN` names your own binary. Keep them in the app you distribute.

`inca package` doesn't sign the app yet. macOS Gatekeeper will warn
before it opens.

## Run log <Badge type="warning" text="unreleased"/>

A packaged app has no terminal, so it writes its errors to a log file. Each run
writes one file. The first error or panic of the run creates it, and a run
without one leaves no file.

The directory depends on the platform:

- macOS: `~/Library/Logs/<identifier>/`
- Linux: `$XDG_STATE_HOME/<identifier>/`, or `~/.local/state/<identifier>/` when
  `XDG_STATE_HOME` is unset or not an absolute path

`<identifier>` is the `identifier` of your config. An identifier that is empty,
starts with `.` or has a character other than an ASCII letter, a digit, `.`,
`-` or `_` gives `inca-app`. A config with no identifier gives `inca-app` too.

Without a home directory, macOS uses the temporary directory. Linux also uses it
when there is no home directory and `XDG_STATE_HOME` is unset or not an
absolute path.

The file is named after the UTC start of the run, for example
`20261102T163000Z.log`. Each entry has a time, a kind and the message with its
stack:

- `error`: an uncaught error in a listener, an unhandled promise rejection or a
  script stopped by the memory or stack limit
- `startup`: the error that stops the first load. The other errors of that load
  are `error` entries before it.
- `panic`: a panic of the host, always the last entry. `RUST_BACKTRACE=1` adds a
  backtrace.
- `notice`: the run reached the size limit

Console output is not recorded. The app keeps the 10 newest files. A run writes
at most 1 MiB of errors, then adds a `notice` and drops later errors. A panic
is still written.
