<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Changelog

Follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- the event object has `composedPath()`
- the event object carries `bubbles`, `cancelable`, `composed`, `defaultPrevented`, `eventPhase`, `isTrusted` and `timeStamp`
- the `button` element has a default `tabindex` of 0, fires `click` from Enter and Space, and takes a `disabled` attribute
- `focusin` and `focusout` events, and `relatedTarget` on `focus`, `blur`, `focusin` and `focusout`
- `el.tabIndex` sets the `tabindex` attribute ([9f63f4a](https://github.com/tom96da/incajs/commit/9f63f4a))

### Changed

- a focused node that gets `hidden`, `inert` or `display: none`, or sits under an ancestor that does, loses focus
- `hidden` renders as `display: none`, and an `inert` subtree receives no mouse events
- `event.target` is the node the event started on, so a listener on an ancestor can delegate, `.self` runs only for the node itself, and `eventPhase` is 2 on that node and 3 above it
- Enter and Space fire `click` on a focused `button` only
- a node takes focus through its `tabindex` attribute, and `focus()` on other nodes is ignored ([9f63f4a](https://github.com/tom96da/incajs/commit/9f63f4a))
- a click outside every `tabindex` node blurs the focused node ([9f63f4a](https://github.com/tom96da/incajs/commit/9f63f4a))

### Fixed

- with `--experimental-hmr`, the last of two saves made within 50 ms of each other is applied ([9df1cce](https://github.com/tom96da/incajs/commit/9df1cce))

## [0.0.11] - 2026-10-06

### Added

- `src/main.ts` and `src/main.js` work as app entries after `src/main.mts` ([c488257](https://github.com/tom96da/incajs/commit/c488257))
- the host prints a warning line when a style sets an unknown key or an invalid color ([255e580](https://github.com/tom96da/incajs/commit/255e580))
- templates accept event and key modifiers such as `.stop` and `.enter`, with some limits for now ([a0cb74d](https://github.com/tom96da/incajs/commit/a0cb74d))

### Changed

- `@vue/runtime-dom` becomes a peer dependency of `incajs` ([a0cb74d](https://github.com/tom96da/incajs/commit/a0cb74d))
- `console` prints the first 100 entries of an array, `Map` or `Set` and counts the rest ([2ba40c3](https://github.com/tom96da/incajs/commit/2ba40c3))

### Fixed

- a root that sets only a width or only a height sizes that axis of the window ([e2ec650](https://github.com/tom96da/incajs/commit/e2ec650))
- a signed hex color and a number outside `0` to `0xffffff` are rejected ([255e580](https://github.com/tom96da/incajs/commit/255e580))
- `inca dev` uses only the settings the current config declares ([0ab33ad](https://github.com/tom96da/incajs/commit/0ab33ad))
- a malformed `package.json` fails with an error that names the file ([769d777](https://github.com/tom96da/incajs/commit/769d777))
- a prop that turns `null` or `undefined`, or leaves the template, is removed from the element ([a944795](https://github.com/tom96da/incajs/commit/a944795))
- `v-model` on an element registers listeners only for the events the template names ([4df3e50](https://github.com/tom96da/incajs/commit/4df3e50))
- `defineConfig` provides the types of the config keys, which the package had lost ([53ce7de](https://github.com/tom96da/incajs/commit/53ce7de))

## [0.0.10] - 2026-10-05

### Changed

- the host binary is about 40% smaller ([28c6b3c](https://github.com/tom96da/incajs/commit/28c6b3c), [6b5b423](https://github.com/tom96da/incajs/commit/6b5b423))
- an `onErrorCaptured` hook that returns `false` keeps a handler error out of the host's error report ([20fc77a](https://github.com/tom96da/incajs/commit/20fc77a))

### Fixed

- every failing handler of one event reaches the host's error report ([20fc77a](https://github.com/tom96da/incajs/commit/20fc77a))
- a listener added while an event is being handled first runs for the next event ([f83f2dd](https://github.com/tom96da/incajs/commit/f83f2dd))
- a `<slot/>` or any fragment in a container adds no blank line above its content ([5f4c214](https://github.com/tom96da/incajs/commit/5f4c214))
- a failed HMR update prints the same message and error code as a failed rebuild ([d998c2a](https://github.com/tom96da/incajs/commit/d998c2a))
- `console` prints a getter or setter as `[Getter]` or `[Setter]` and a `Proxy` as its target, with their code left idle ([2df1df1](https://github.com/tom96da/incajs/commit/2df1df1))
- `console` prints array holes as `<1 empty item>` and arrays of any length ([2df1df1](https://github.com/tom96da/incajs/commit/2df1df1))
- `console` quotes a key that is not an identifier and keeps the sign of `-0` under `%d` and `%s` ([2df1df1](https://github.com/tom96da/incajs/commit/2df1df1))
- `movementX`/`movementY` are measured from the last pointer move, and a `mouseenter` or `mouseleave` that no move caused reports 0 ([aa9567c](https://github.com/tom96da/incajs/commit/aa9567c))
- `inca dev` keeps running when the config or the entry fails before the first build, prints the error, and retries on the next save ([bfd2282](https://github.com/tom96da/incajs/commit/bfd2282))

## [0.0.9] - 2026-10-03

### Added

- `console` has every standard method, the `%` format specifiers, more value types and colour on a terminal ([fd8c2a9](https://github.com/tom96da/incajs/commit/fd8c2a9), [6d47dc9](https://github.com/tom96da/incajs/commit/6d47dc9), [d4b4a08](https://github.com/tom96da/incajs/commit/d4b4a08), [e3ec293](https://github.com/tom96da/incajs/commit/e3ec293))
- the `@incajs/host-darwin-x64` package is published, so the host runs on Intel Macs ([9ddc458](https://github.com/tom96da/incajs/commit/9ddc458))

### Changed

- the documentation site moved to https://incajs.tom96da.com/ ([e391c46](https://github.com/tom96da/incajs/commit/e391c46))

### Fixed

- a `click.once` handler removes only itself and no longer removes the `click` handlers on the same element ([5d5a29b](https://github.com/tom96da/incajs/commit/5d5a29b))
- `stopImmediatePropagation()` on a `click` event stops the later handlers on the same element ([5d5a29b](https://github.com/tom96da/incajs/commit/5d5a29b))
- the entry bundle runs once even when another file imports it by a different path ([b163ebb](https://github.com/tom96da/incajs/commit/b163ebb))

## [0.0.8] - 2026-09-30

### Added

- `overflow`/`overflow_x`/`overflow_y` accept `visible`, `hidden`, `scroll` and `auto`; scroll containers scroll with the wheel, `stopPropagation()` on `wheel` does not stop the scroll, `preventDefault()` cancels it, and scrollbars are not supported ([a811daa](https://github.com/tom96da/incajs/commit/a811daa), [cc992f7](https://github.com/tom96da/incajs/commit/cc992f7))
- box style keys: `padding`, `margin`, `flex_grow`, `flex_shrink`, `opacity` and `min_*`/`max_*` sizes ([09ddef4](https://github.com/tom96da/incajs/commit/09ddef4))

### Fixed

- a template with many static elements mounts without crashing ([826f187](https://github.com/tom96da/incajs/commit/826f187))
- `inca dev` and `inca build` refuse an `outDir` that reaches the app's files through a symlink ([5951eff](https://github.com/tom96da/incajs/commit/5951eff))
- `inca dev` reports a missing host binary as an error and stops before building ([5a4b287](https://github.com/tom96da/incajs/commit/5a4b287), [033cacf](https://github.com/tom96da/incajs/commit/033cacf))
- `inca dev` exits with an error when the host crashes after it was ready ([0cbbd0b](https://github.com/tom96da/incajs/commit/0cbbd0b))
- a job that throws while a module awaits at the top level is reported ([68ff9d2](https://github.com/tom96da/incajs/commit/68ff9d2))
- `mouseenter` and `mouseleave` report the same `movementX`/`movementY` as `mousemove` for one pointer move ([aa31166](https://github.com/tom96da/incajs/commit/aa31166))
- text inside a `<text>` element's children is rendered ([2c7ea9a](https://github.com/tom96da/incajs/commit/2c7ea9a))

## [0.0.7] - 2026-09-29

### Added

- `inca dev` and `inca build` refuse an `outDir` that holds the app's own files ([8dfe80a](https://github.com/tom96da/incajs/commit/8dfe80a))
- `inca dev` exits with an error when the host exits before it is ready ([1fe4f0f](https://github.com/tom96da/incajs/commit/1fe4f0f))
- `inca package` refuses a `productName` that collides with a build output entry ([0e5e518](https://github.com/tom96da/incajs/commit/0e5e518))

### Changed

- wheel `deltaX`/`deltaY` are positive when scrolling down or right, as in the DOM (previously the opposite) ([249c2d6](https://github.com/tom96da/incajs/commit/249c2d6))

### Fixed

- a host startup failure exits with code 1 and prints its queued output ([c730808](https://github.com/tom96da/incajs/commit/c730808))
- an error thrown while the host loads the app is reported ([1fe4f0f](https://github.com/tom96da/incajs/commit/1fe4f0f))
- inserting an element before itself no longer moves it ([249c2d6](https://github.com/tom96da/incajs/commit/249c2d6))
- `el.blur()` no longer removes focus from a different element ([71e9f39](https://github.com/tom96da/incajs/commit/71e9f39))
- `movementX`/`movementY` are no longer 0 for the second listener a pointer event bubbles to ([71e9f39](https://github.com/tom96da/incajs/commit/71e9f39))
- errors from `@event` handlers go through Vue's error handling and are still reported to the host ([874f120](https://github.com/tom96da/incajs/commit/874f120))
- errors thrown in a promise callback and unhandled promise rejections are reported as application errors ([24ff686](https://github.com/tom96da/incajs/commit/24ff686))
- a style key removed from a `:style` binding reverts to its default ([923860c](https://github.com/tom96da/incajs/commit/923860c))

## [0.0.6] - 2026-09-28

### Added

- experimental HMR (hot module reloading) became available behind `inca dev --experimental-hmr` ([0f54d9c](https://github.com/tom96da/incajs/commit/0f54d9c))

### Fixed

- right and middle mouse buttons report the correct DOM `buttons` bitmask ([b7803bb](https://github.com/tom96da/incajs/commit/b7803bb))
- a module reached by two different relative import paths evaluates once instead of twice ([acf8042](https://github.com/tom96da/incajs/commit/acf8042))
- a resizable window's size no longer snaps back to content size on reload ([d556bc2](https://github.com/tom96da/incajs/commit/d556bc2))
- `.once` event modifiers really unbind after firing, instead of registering a dead listener ([23a63eb](https://github.com/tom96da/incajs/commit/23a63eb))
- `.passive`/`.capture` event modifiers no longer break the listener ([23a63eb](https://github.com/tom96da/incajs/commit/23a63eb))

## [0.0.5] - 2026-09-26

### Added

- DOM-shaped `pageX`/`pageY`/`movementX`/`movementY` payloads became available on pointer events ([4548c9f](https://github.com/tom96da/incajs/commit/4548c9f))
- a template `ref`'s element carries `.focus()`/`.blur()` directly ([0258564](https://github.com/tom96da/incajs/commit/0258564))

### Removed

- `incajs`'s tree/event/focus functions are no longer exported from its root; use a template `ref`'s `.focus()`/`.blur()` instead ([fa36a6a](https://github.com/tom96da/incajs/commit/fa36a6a))
- `@incajs/cli`'s unused bundler-adapter type exports were dropped from its root ([ca5da96](https://github.com/tom96da/incajs/commit/ca5da96))

### Fixed

- `inca dev` no longer hangs on Ctrl-C when a reload is waiting on a wedged host ([a8f2bdf](https://github.com/tom96da/incajs/commit/a8f2bdf))
- `console.log` prints `Infinity`/`-Infinity` the way JS spells them, not Rust's `inf`/`-inf` ([c589fe2](https://github.com/tom96da/incajs/commit/c589fe2))

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

- every failure carries an `ERR_INCA_*` code, listed in the new [error reference](https://incajs.tom96da.com/reference/errors) ([7a2e6e4](https://github.com/tom96da/incajs/commit/7a2e6e4))
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

[Unreleased]: https://github.com/tom96da/incajs/compare/v0.0.11...HEAD
[0.0.11]: https://github.com/tom96da/incajs/releases/tag/v0.0.11
[0.0.10]: https://github.com/tom96da/incajs/releases/tag/v0.0.10
[0.0.9]: https://github.com/tom96da/incajs/releases/tag/v0.0.9
[0.0.8]: https://github.com/tom96da/incajs/releases/tag/v0.0.8
[0.0.7]: https://github.com/tom96da/incajs/releases/tag/v0.0.7
[0.0.6]: https://github.com/tom96da/incajs/releases/tag/v0.0.6
[0.0.5]: https://github.com/tom96da/incajs/releases/tag/v0.0.5
[0.0.4]: https://github.com/tom96da/incajs/releases/tag/v0.0.4
[0.0.3]: https://github.com/tom96da/incajs/releases/tag/v0.0.3
[0.0.2]: https://github.com/tom96da/incajs/releases/tag/v0.0.2
[0.0.1]: https://github.com/tom96da/incajs/releases/tag/v0.0.1
