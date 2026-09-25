<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Changelog

Follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.4] - 2026-09-25

### Added

- `inca dev` runs the app under the `productName`, `icon` and `identifier` from `inca.config.ts` ([b78d43a](https://github.com/tom96da/incajs/commit/b78d43a))
- `inca.config.ts` takes a `window` block: `width`, `height`, `title`, `resizable`, `minWidth` and `minHeight` ([c35e6ea](https://github.com/tom96da/incajs/commit/c35e6ea), [4b6978e](https://github.com/tom96da/incajs/commit/4b6978e))
- the window's title bar carries `window.title`, or the app's `productName` ([c35e6ea](https://github.com/tom96da/incajs/commit/c35e6ea))
- an app's `identifier` becomes its Wayland `app_id` and X11 `WM_CLASS` ([c35e6ea](https://github.com/tom96da/incajs/commit/c35e6ea))
- a `productName` that can't name a directory fails with `ERR_INCA_PRODUCT_NAME_INVALID` ([fb8fe0f](https://github.com/tom96da/incajs/commit/fb8fe0f))
- every app gets an application menu with a `Quit` item, on `⌘Q` on macOS and `Ctrl+Q` on Linux ([7db49c8](https://github.com/tom96da/incajs/commit/7db49c8))
- DOM-shaped `"mousedown"`/`"mouseup"`/`"mousemove"` payloads became available (position, button, modifiers) ([4247c00](https://github.com/tom96da/incajs/commit/4247c00), [6364e86](https://github.com/tom96da/incajs/commit/6364e86))
- DOM-shaped `"wheel"`/`"mouseenter"`/`"mouseleave"` payloads became available ([29b111c](https://github.com/tom96da/incajs/commit/29b111c), [6dce804](https://github.com/tom96da/incajs/commit/6dce804))
- `stopPropagation()`/`stopImmediatePropagation()`/`preventDefault()` became available on every event ([6364e86](https://github.com/tom96da/incajs/commit/6364e86))
- `focusNode`/`blurNode` and `"focus"`/`"blur"` became available ([c088112](https://github.com/tom96da/incajs/commit/c088112))
- `"keydown"`/`"keyup"` fire on the focused node and bubble to its ancestors, DOM-`KeyboardEvent`-shaped ([b94c74b](https://github.com/tom96da/incajs/commit/b94c74b))

### Changed

- a window is not resizable unless `window.resizable` is `true` ([4b6978e](https://github.com/tom96da/incajs/commit/4b6978e))

### Fixed

- `inca package` no longer deletes the build output for an app whose name has no ASCII letters or digits ([be3a423](https://github.com/tom96da/incajs/commit/be3a423))
- `inca dev` no longer reports itself as already running after a failed start ([be3a423](https://github.com/tom96da/incajs/commit/be3a423))

## [0.0.3] - 2026-09-20

### Added

- every failure carries an `ERR_INCA_*` code, listed in the new [error reference](https://tom96da.github.io/incajs/reference/errors) ([7a2e6e4](https://github.com/tom96da/incajs/commit/7a2e6e4))
- a failed build is reported once, with the bundler's own excerpt ([e5a177d](https://github.com/tom96da/incajs/commit/e5a177d))
- a `<style>` block or stylesheet import fails the build instead of being ignored ([ecabe7b](https://github.com/tom96da/incajs/commit/ecabe7b))
- `inca dev`, `build` and `package` show the bundler's own build output ([bafa985](https://github.com/tom96da/incajs/commit/bafa985))
- `inca dev` names the file behind each reload, and how long it took ([c808782](https://github.com/tom96da/incajs/commit/c808782))
- `inca dev` refuses to start when one is already open for the app ([e416bdb](https://github.com/tom96da/incajs/commit/e416bdb))

### Changed

- the Linux host binaries are built against glibc 2.35, so Ubuntu 22.04 and Debian 12 can run them ([a7a468a](https://github.com/tom96da/incajs/commit/a7a468a), [6512bbb](https://github.com/tom96da/incajs/commit/6512bbb))

### Fixed

- `inca dev` no longer crashes when the host exits mid-shutdown (`EPIPE`) ([fb93059](https://github.com/tom96da/incajs/commit/fb93059))
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

[Unreleased]: https://github.com/tom96da/incajs/compare/v0.0.4...HEAD
[0.0.4]: https://github.com/tom96da/incajs/releases/tag/v0.0.4
[0.0.3]: https://github.com/tom96da/incajs/releases/tag/v0.0.3
[0.0.2]: https://github.com/tom96da/incajs/releases/tag/v0.0.2
[0.0.1]: https://github.com/tom96da/incajs/releases/tag/v0.0.1
