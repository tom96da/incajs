---
description: Common problems when running an Incarnative.js app, and where to look.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Troubleshooting

## Where does `console` output go?

`inca dev` relays the host's stderr, so your app's `console.log`
(and the host's own reports) show up in the same terminal you ran
`inca dev` from.

A packaged app (`inca package`) has no such relay — a double-clicked
`.app` writes its `console` output to a stderr nobody reads, so it's
lost. This is a known gap; see
[BACKLOG.md](https://github.com/tom96da/incajs/blob/main/specs/BACKLOG.md)
for the current state.

## The app won't start on Linux

If the machine is missing a required library or a Vulkan driver, the
app fails to start rather than falling back to a software renderer. See
[Linux runtime requirements](./#linux-runtime-requirements) for the
libraries and driver package it needs, and how to check for them.
