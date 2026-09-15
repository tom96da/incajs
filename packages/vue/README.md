<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# @incajs/vue

Vue 3 custom renderer for [`incajs`](https://github.com/tom96da/incajs) —
maps `@vue/runtime-core`'s `createRenderer` lifecycle methods onto the
native host bridge.

It never imports `incajs` itself. `createIncaApp` takes an `IncaCore`
object — the same shape `incajs`'s own exports have — as its first
argument, and calls that instead. This keeps the two packages decoupled
(no dependency either way) while still letting `incajs`'s own `incajs/vue`
subpath pre-wire this renderer with its own exports, for callers who'd
rather not pass that argument themselves.
