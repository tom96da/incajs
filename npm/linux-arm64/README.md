<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# @incajs/host-linux-arm64

Prebuilt `inca-host` binary for Linux on arm64. Not meant to be depended
on directly — `@incajs/cli`'s dev-client installs whichever of these matches
the current OS/arch as an `optionalDependency` and resolves the binary
inside it.

This package carries no JS: `bin/inca-host` is the whole of it.

Part of [tom96da/incajs](https://github.com/tom96da/incajs), the
Incarnative.js framework.