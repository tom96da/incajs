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
