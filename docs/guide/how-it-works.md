---
description: The pipeline inca runs, from your .vue source to pixels on screen, and why it's built that way.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# How it Works

**Incarnative.js** renders your app without a browser engine. There's
no Chromium, no DOM, and no webview anywhere in the picture — just your
`.vue` components, a native runtime, and the GPU.

## The pipeline

`inca dev`/`build`/`package` all start the same way: your app is bundled
with [Vite](https://vite.dev/), the current bundler adapter, compiling
`.vue` templates and scripts down to plain JavaScript. Vue is likewise the
current, first-class frontend framework — the architecture leaves room for
others later, but none are implemented yet.

For `dev` and `package`, that bundle is then handed to the host — a
native process, written in Rust on top of [GPUI](https://www.gpui.rs/) —
which runs it and renders it:

1. **Vite** bundles your `.vue`/`.mts` source.
2. **QuickJS**, a small embedded JS engine, runs the bundle inside the
   host. This is where Vue's reactivity and your component logic
   execute — there's no browser JS engine, and no Node.js in the packaged
   app.
3. A custom Vue renderer turns your component tree into calls against a
   small native bridge (create a node, set a style, append a child, …)
   instead of DOM operations.
4. **GPUI** keeps a retained tree from those calls and draws it directly
   to the GPU — no HTML/CSS layout engine, no compositor in between.

## Why this shape

Your app's JS only runs when something actually changes — a reactive
update, an event handler. It pushes the resulting mutations to the native
side once, and GPUI owns layout and painting from there. Nothing re-runs
JS every frame the way a browser's render loop would.

Because there's no browser engine, `.vue` authoring uses a native styling
vocabulary rather than CSS — see
[Elements & Styles](../reference/elements) for what's supported today.

Ready to build one? See [Getting Started](./).
