---
description: Every ERR_INCA_* code the inca CLI reports, what causes it, and how to resolve it.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Error Codes

Every failure the CLI raises itself carries an `ERR_INCA_*` code, printed
with the message:

```
[inca] build failed (ERR_INCA_ENTRY_NOT_FOUND): no app entry found — expected …
```

A failure with no code came from somewhere else — the bundler, the host,
or Node itself — and its own message is what to search for.

## `ERR_INCA_ENTRY_NOT_FOUND`

No entry point. Incarnative.js looks for `src/main.mts`, then `src/App.vue` (which
it wraps in an entry for you).

Create one of those, or point `entry` in `inca.config.ts` at the file you
use instead.

## `ERR_INCA_PACKAGE_JSON_NOT_FOUND`

`inca package` requires the app's `package.json`, where it reads the name
and version, and there is none in the directory it ran in. Run it from
the app's own root, or add the file.

## `ERR_INCA_PRODUCT_NAME_MISSING`

The packaged app has nothing to be called. The name comes from
`productName` in `inca.config.ts`, and failing that from `package.json`'s
own top-level `name` — the npm package name, with any scope stripped, so
`@acme/todo` packages as `todo`. Neither is set.

Set [`productName`](./configuration#productname) when the app's display
name isn't its package name.

## `ERR_INCA_PRODUCT_NAME_INVALID`

The app's name becomes a directory name: `<productName>.app` on macOS, and
a scratch bundle under `node_modules/.inca` while `inca dev` runs. See
[`productName`](./configuration#productname) for the naming rule this
enforces.

`inca package` raises it too when the app's directory would share a name
with a top-level file or folder of the build output, such as `assets` or
`chunks`. Pick a different `productName`.

## `ERR_INCA_ICON_NOT_FOUND`

`icon` in `inca.config.ts` points at a `.icns` file that doesn't exist. The
path is resolved from the app's root directory. `inca dev` retries when the
file is created.

## `ERR_INCA_OUT_DIR_INVALID`

[`outDir`](./configuration#outdir) is, or contains, the app's root
directory, its `src/` or `node_modules/`, or its configured
[`entry`](./configuration#entry), including through a symlink. `inca dev`
and `inca build` delete every file under `outDir` that the build didn't
write. Set it to a directory the build owns, such as `"dist"`.

## `ERR_INCA_PLATFORM_UNSUPPORTED`

`inca package` runs on macOS and Linux. Windows isn't supported yet.

## `ERR_INCA_HOST_BIN_NOT_FOUND`

`inca dev` or `inca package` found no file at the host binary's path.
`inca dev` stops before it builds anything.

Check the path `INCA_HOST_BIN` points at.

## `ERR_INCA_HOST_BIN_UNRESOLVED`

No host binary could be found at all: the `@incajs/host-*` package for
this platform isn't installed, and `INCA_HOST_BIN` isn't set.
`inca dev` stops here too.

Install the app's dependencies, or set `INCA_HOST_BIN` to a binary you
built yourself.

> [!NOTE]
> Windows has no published package yet — see
> [Host binary](../guide/#host-binary) for platform support.

## `ERR_INCA_DEV_RUNNING`

Another `inca dev` already has this app open — its pid is in the message.
Two of them watch the same files and write to the same terminal, so the
second one refuses to start.

Quit the first, or stop the process by that pid if it was left behind.

## `ERR_INCA_HOST_EXITED_EARLY`

The host exited before it reported ready, and `inca dev` exits with a
non-zero code. Usually your app threw while the host loaded it, or the host
speaks a protocol revision this `@incajs/cli` wasn't built for. It can also
be a host that can't start at all, such as a missing Vulkan driver (see
[Troubleshooting](../guide/troubleshooting)). The line printed just before
says which. Fix the error in your app, or install matching `@incajs/cli`
and `@incajs/host-*` versions.

## `ERR_INCA_HOST_CRASHED`

The host exited after it reported ready with a non-zero code, or was ended
by a signal other than SIGINT or SIGTERM. `inca dev` exits with a non-zero
code. The message gives the exit code or the signal.

The host's own output, printed before the exit, says what went wrong. Report
it as a bug if your app didn't cause it.

## `ERR_INCA_UNSUPPORTED_STYLE`

A `.vue` file has a `<style>` block, which isn't supported yet — the
renderer applies no stylesheet, so the block would have no effect at
runtime, and the build stops rather than shipping it.

Use a `:style` binding instead.

Not supported yet:

```vue
<style>
.box { color: red; }
</style>
```

Use instead:

```vue
<template>
  <div :style="{ text_color: '#ff0000' }">…</div>
</template>
```

## `ERR_INCA_UNSUPPORTED_STYLESHEET`

Something imports a `.css` (or `.scss`, `.less`, …) file. Same reason as
above — see `ERR_INCA_UNSUPPORTED_STYLE`.

## `ERR_INCA_BUILD_NO_OUTPUT`

The bundler finished without writing anything. This one is a bug in
Incarnative.js rather than in your app — please
[open an issue](https://github.com/tom96da/incajs/issues).

## `ERR_INCA_HMR_UNSUPPORTED`

`inca dev --experimental-hmr` (or `INCA_EXPERIMENTAL_HMR`) was set, but the
configured bundler has no HMR support. The default bundler supports it;
this only happens with a custom `bundler` that doesn't implement `hmr`.

## `ERR_INCA_BUILD_NO_ENTRY_CHUNK`

The bundler wrote files, but none of them was marked as the entry, so
there is nothing for the host to evaluate. A bug in Incarnative.js too —
please [open an issue](https://github.com/tom96da/incajs/issues).
