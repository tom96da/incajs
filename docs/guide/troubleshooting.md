---
description: Common problems when running an Incarnative.js app, and where to look.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Troubleshooting

## Where does `console` output go?

The development window shows your app's `console` output in the terminal
that ran `inca dev`. See the [Console reference](../reference/console)
for where each kind of app writes it.

## The app won't start on Linux

If the machine is missing a required library or a Vulkan driver, the
app fails to start rather than falling back to a software renderer. See
[Linux runtime requirements](./#linux-runtime-requirements) for the
libraries and driver package it needs, and how to check for them.

## Where does a packaged app write its crash report? <Badge type="warning" text="unreleased"/>

When the host panics, a packaged app writes the panic to `panic.log` and ends.
On macOS the file is `~/Library/Logs/<identifier>/panic.log`. On Linux it is
`$XDG_STATE_HOME/<identifier>/panic.log`, or
`~/.local/state/<identifier>/panic.log` when `XDG_STATE_HOME` is not set, is
empty or is not an absolute path. When the host finds no home directory, the
file is `<identifier>/panic.log` in the temporary directory.
`<identifier>` is the `identifier` of your config. The directory is `inca-app`
for an app with no identifier and for an identifier that is empty, starts with
`.` or has a character other than an ASCII letter, a digit, `.`, `-` or `_`.
The file holds the last panic. Start the app with `RUST_BACKTRACE=1` to add a
backtrace.

## Why does my script throw an out-of-memory or a stack error? <Badge type="warning" text="unreleased"/>

The host limits a script to 1 GiB of heap and 6 MiB of stack. An allocation
past the heap limit throws `InternalError: out of memory`, and recursion past
the stack limit throws `RangeError: Maximum call stack size exceeded`. The
engine can throw `null` where it cannot allocate an error object, and the host
reports that as `null. the engine also throws null when it runs out of memory`.

## Why doesn't my content scroll when it is taller than the window?

Content taller than the window is cut off at the window edge. Put the content
in an element with a `height` and `overflow: "scroll"`, and the wheel scrolls
that element:

```vue
<template>
  <div :style="{ overflow: 'scroll', height: 400 }">
    <!-- content taller than 400px -->
  </div>
</template>
```

[`overflow`](../reference/elements) lists the other values.
