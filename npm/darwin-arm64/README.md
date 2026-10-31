<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# @incajs/host-darwin-arm64

Prebuilt `inca-host` binary for macOS on Apple Silicon (arm64). Not
meant to be depended on directly. `@incajs/cli` installs whichever of these
matches the current OS/arch as an `optionalDependency` and unpacks the
binary into a cache directory.

This package carries no JS: `bin/inca-host.br`, the brotli-compressed
binary, is the whole of it.

Part of [tom96da/incajs](https://github.com/tom96da/incajs), the
Incarnative.js framework.