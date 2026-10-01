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

`console` output is coloured when it goes to a terminal
<Badge type="warning" text="unreleased" />. Set `NO_COLOR` to any
non-empty value to turn the colours off, or `FORCE_COLOR` to a value
other than `0` or `false` to turn them on when the output is piped.
`NO_COLOR` wins when both are set.

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
