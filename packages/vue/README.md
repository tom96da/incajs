<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# @incajs/vue

Vue 3 custom renderer for [`incajs`](https://github.com/tom96da/incajs) —
maps `@vue/runtime-core`'s `createRenderer` lifecycle methods onto
`incajs`'s typed wrapper functions over the native host bridge.

Also reachable as the `incajs/vue` subpath of the `incajs` package itself,
for callers who'd rather not add a second dependency.
