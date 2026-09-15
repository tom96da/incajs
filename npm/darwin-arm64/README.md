<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# @incajs/host-darwin-arm64

Prebuilt `inca-host` binary for macOS on Apple Silicon (arm64). Not
meant to be depended on directly — `@incajs/cli`'s dev-client installs
whichever of these matches the current OS/arch as an `optionalDependency`
and resolves the binary inside it.

This package carries no JS: `bin/inca-host` is the whole of it.