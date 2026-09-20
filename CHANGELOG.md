<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- every failure carries an `ERR_INCA_*` code, listed in the new [error reference](https://tom96da.github.io/incajs/reference/errors) ([7a2e6e4](https://github.com/tom96da/incajs/commit/7a2e6e4))
- a failed build is reported once, with the excerpt the bundler pointed at ([e5a177d](https://github.com/tom96da/incajs/commit/e5a177d))
- a `<style>` block or stylesheet import fails the build, instead of being ignored at runtime ([ecabe7b](https://github.com/tom96da/incajs/commit/ecabe7b))
- `inca dev`, `build` and `package` show the bundler's own build output ([bafa985](https://github.com/tom96da/incajs/commit/bafa985))
- `inca dev` names the file that triggered a reload, and how long it took ([c808782](https://github.com/tom96da/incajs/commit/c808782))
- `inca dev` exits when a dev window is already open for the same app ([e416bdb](https://github.com/tom96da/incajs/commit/e416bdb))

### Fixed

- `inca dev` no longer crashes with `EPIPE` when the host exits first, mid-shutdown ([fb93059](https://github.com/tom96da/incajs/commit/fb93059))
- closing the last window quits the app and exits `inca dev` ([ad1d800](https://github.com/tom96da/incajs/commit/ad1d800), [4763659](https://github.com/tom96da/incajs/commit/4763659))

## [0.0.2] - 2026-09-20

### Added

- `inca.config.ts` for app metadata and build settings, with a `defineConfig` helper from `@incajs/cli/config` ([9e74511](https://github.com/tom96da/incajs/commit/9e74511))

### Changed

- Node.js 22.18 or newer is now required ([9e74511](https://github.com/tom96da/incajs/commit/9e74511))
- updated GPUI and QuickJS to their latest releases ([efb37af](https://github.com/tom96da/incajs/commit/efb37af))

### Deprecated

- `package.json`'s `"inca"` key, superseded by `inca.config.ts` ([9e74511](https://github.com/tom96da/incajs/commit/9e74511))

## [0.0.1] - 2026-09-18

### Features

- GPU-native rendering via [GPUI](https://www.gpui.rs/), no Chromium or webview
- embedded QuickJS runtime ([`rquickjs`](https://github.com/DelSkayn/rquickjs)) for sub-second startup
- first-class Vue 3 support via the `incajs/vue` custom renderer
- `inca` CLI: `dev` (live reload), `build`, and `package` (a distributable `.app` on macOS, a plain directory on Linux)
- prebuilt `inca-host` binaries for linux-x64, linux-arm64, and darwin-arm64, resolved automatically per platform
- click event dispatch from the native tree back into JS

[Unreleased]: https://github.com/tom96da/incajs/compare/v0.0.2...HEAD
[0.0.2]: https://github.com/tom96da/incajs/releases/tag/v0.0.2
[0.0.1]: https://github.com/tom96da/incajs/releases/tag/v0.0.1
