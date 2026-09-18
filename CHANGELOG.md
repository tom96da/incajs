<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1] - 2026-09-18

### Features

- GPU-native rendering via [GPUI](https://www.gpui.rs/), no Chromium or
  webview
- embedded QuickJS runtime ([`rquickjs`](https://github.com/DelSkayn/rquickjs))
  for sub-second startup
- first-class Vue 3 support via the `incajs/vue` custom renderer
- `inca` CLI: `dev` (live reload), `build`, and `package` (a distributable
  `.app` on macOS, a plain directory on Linux)
- prebuilt `inca-host` binaries for linux-x64, linux-arm64, and
  darwin-arm64, resolved automatically per platform
- click event dispatch from the native tree back into JS

[Unreleased]: https://github.com/tom96da/incajs/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/tom96da/incajs/releases/tag/v0.0.1
