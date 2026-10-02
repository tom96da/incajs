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
