<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Changelog

Follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- pointer events and `setPointerCapture()` on elements ([404158a](https://github.com/tom96da/incajs/commit/404158a))
- `document` between the elements and `window` in every event path, imported from `incajs` ([c1f21f6](https://github.com/tom96da/incajs/commit/c1f21f6))
- `window` and `document` have handler properties such as `onkeydown` ([c1f21f6](https://github.com/tom96da/incajs/commit/c1f21f6))
- `window` receives `focus` and `blur` when the window is activated or deactivated ([c1f21f6](https://github.com/tom96da/incajs/commit/c1f21f6))
- `contextmenu` from the Menu key, and from Shift+F10 outside macOS ([404158a](https://github.com/tom96da/incajs/commit/404158a))

### Changed

- the mount container is an event target, and `composedPath()` ends with `document` and `window` ([c1f21f6](https://github.com/tom96da/incajs/commit/c1f21f6))
- a `wheel` listener on the mount container is passive unless `passive: false` is given ([c1f21f6](https://github.com/tom96da/incajs/commit/c1f21f6))
- `click`, `auxclick` and `contextmenu` report `isPrimary` false and `pressure` 0 ([404158a](https://github.com/tom96da/incajs/commit/404158a))
- the `stack` an error carries names a recompiled module with a `?t=` query ([04fe222](https://github.com/tom96da/incajs/commit/04fe222))

### Fixed

- a function of an earlier compile keeps its source locations after a recompile ([04fe222](https://github.com/tom96da/incajs/commit/04fe222))
- `inca dev` reloads the window when its automatically selected entry is added or deleted ([e224a27](https://github.com/tom96da/incajs/commit/e224a27), [04ec539](https://github.com/tom96da/incajs/commit/04ec539), [addf498](https://github.com/tom96da/incajs/commit/addf498))
- a panic no longer hangs the host while `inca dev` stops reading its output ([465a02d](https://github.com/tom96da/incajs/commit/465a02d))

## [0.0.21] - 2026-10-21

### Added

- `MouseEvent`, `KeyboardEvent` and the other event classes can be built and dispatched ([2f531d5](https://github.com/tom96da/incajs/commit/2f531d5))
- key events carry `keyCode`, `which` and `charCode`, and UI events `detail` and `view` ([aa0d2ea](https://github.com/tom96da/incajs/commit/aa0d2ea))
- listener and handler parameters are typed by the event name, and `this` by the target ([630382d](https://github.com/tom96da/incajs/commit/630382d))
- `console` prints the state of a promise and the elements of a typed array ([5f2745c](https://github.com/tom96da/incajs/commit/5f2745c))

### Fixed

- Tab after a click or a blur continues from that element, also after it leaves the tree ([4648d47](https://github.com/tom96da/incajs/commit/4648d47))
- the host exits 1 with a message when `DISPLAY` or `WAYLAND_DISPLAY` names no running server ([04b8c8c](https://github.com/tom96da/incajs/commit/04b8c8c))
- the experimental HMR update line names an edited file relative to `src` without a main file ([b6030c0](https://github.com/tom96da/incajs/commit/b6030c0))
- `.env` and `public` are read from the app's root directory ([e157133](https://github.com/tom96da/incajs/commit/e157133))

## [0.0.20] - 2026-10-20

### Added

- `inca config` prints the settings the host applies to your app ([466594b](https://github.com/tom96da/incajs/commit/466594b))

### Changed

- an event's fields are getters on its prototype, so `Object.keys(event)` lists only `isTrusted` ([e9ff4dc](https://github.com/tom96da/incajs/commit/e9ff4dc))

### Fixed

- event fields are read-only, and a frozen event can be dispatched again ([e9ff4dc](https://github.com/tom96da/incajs/commit/e9ff4dc))
- `Reflect.construct` with a non-object prototype builds an event that inherits from `Event` ([e9ff4dc](https://github.com/tom96da/incajs/commit/e9ff4dc))
- `stopPropagation()` before `dispatchEvent()` keeps every listener from running ([138a56a](https://github.com/tom96da/incajs/commit/138a56a))
- a boolean attribute bound to `0` or `NaN` is removed, like `false` ([373f34e](https://github.com/tom96da/incajs/commit/373f34e))
- a bordered scroll container scrolls to the end of its padding ([74fd986](https://github.com/tom96da/incajs/commit/74fd986))

## [0.0.19] - 2026-10-18

### Added

- the host prints a warning for a style value it does not support ([d04964f](https://github.com/tom96da/incajs/commit/d04964f))

### Changed

- a negative size, `gap`, `borderWidth`, `cornerRadius` or `textSize` is ignored ([d04964f](https://github.com/tom96da/incajs/commit/d04964f))

### Fixed

- `inca dev` with the experimental HMR prints a `reload` line when an edit reloads the app ([8c2e32e](https://github.com/tom96da/incajs/commit/8c2e32e))
- assigning to a host global leaves events, the development window and rejection reports working ([44acb24](https://github.com/tom96da/incajs/commit/44acb24))
- after `inca dev` retries a failed load, event targets name the app's current elements ([44acb24](https://github.com/tom96da/incajs/commit/44acb24))

## [0.0.18] - 2026-10-18

### Added

- `overflow` takes `clip` and a two-value form such as `hidden scroll` ([2df75fd](https://github.com/tom96da/incajs/commit/2df75fd))
- `onclick` and the other event handler properties on elements ([7c56342](https://github.com/tom96da/incajs/commit/7c56342))
- listener and handler parameters are typed `IncaEvent`, which is exported ([6e8c142](https://github.com/tom96da/incajs/commit/6e8c142))

### Changed

- beside a scrolling axis, `overflow` computes `clip` as `hidden` and an unset axis as `scroll` ([2df75fd](https://github.com/tom96da/incajs/commit/2df75fd))

### Fixed

- an event prop keeps a listener for any value Vue keeps, and `onUpdate:` binds nothing ([cee23c5](https://github.com/tom96da/incajs/commit/cee23c5))
- `inca dev --experimental-hmr` gives the app its `define` values and drops Vue's flag warning ([be6477b](https://github.com/tom96da/incajs/commit/be6477b))

## [0.0.17] - 2026-10-18

### Added

- `contains()` on template refs ([0724fd1](https://github.com/tom96da/incajs/commit/0724fd1))
- `%c` in `console` colors the text after it with a basic `color` ([492ec04](https://github.com/tom96da/incajs/commit/492ec04))

### Changed

- the `id` of a template ref is its `id` attribute, and a text node has no `id` ([8b639b5](https://github.com/tom96da/incajs/commit/8b639b5))
- event `target`, `currentTarget`, `relatedTarget` and `composedPath()` are elements or `window` ([4b3f82a](https://github.com/tom96da/incajs/commit/4b3f82a)) ([57ccbad](https://github.com/tom96da/incajs/commit/57ccbad))
- a listener function runs with the current target as `this` ([5518650](https://github.com/tom96da/incajs/commit/5518650))
- the error of `CustomEvent(...)` called without `new` names `CustomEvent` ([ea5d186](https://github.com/tom96da/incajs/commit/ea5d186))
- the host frees a detached node that nothing holds at the next frame ([37c45fa](https://github.com/tom96da/incajs/commit/37c45fa))
- `inca dev` error messages call the program "the host" ([8ad701b](https://github.com/tom96da/incajs/commit/8ad701b))
- `console` format specifiers are not read again from a substituted value ([492ec04](https://github.com/tom96da/incajs/commit/492ec04))
- the wheel step goes to the first container that can move after the `wheel` listeners ran ([b57a90d](https://github.com/tom96da/incajs/commit/b57a90d))

### Fixed

- events and `activeElement()` find the elements of a mounted app that nothing else references ([86c7421](https://github.com/tom96da/incajs/commit/86c7421))
- `reactive()` keeps `window` from `incajs` as it is ([57ccbad](https://github.com/tom96da/incajs/commit/57ccbad))
- a removed or moved node loses focus at once, and a detached node cannot take it ([ef08420](https://github.com/tom96da/incajs/commit/ef08420))
- the host frees the nodes of a component whose setup threw ([37c45fa](https://github.com/tom96da/incajs/commit/37c45fa))
- a listener prop on a removed element is ignored ([89e61dc](https://github.com/tom96da/incajs/commit/89e61dc))
- `inca dev` reports an entry file the host cannot read like any other start failure ([19daf5e](https://github.com/tom96da/incajs/commit/19daf5e))
- the type declarations of `@incajs/cli` resolve again from the paths `package.json` names ([3a00f91](https://github.com/tom96da/incajs/commit/3a00f91))
- space release clicks a button under a held mouse press, and other keys keep a pending Space press ([bc31fb4](https://github.com/tom96da/incajs/commit/bc31fb4))
- the app hears about a dev message receiver that throws ([b8fa5d0](https://github.com/tom96da/incajs/commit/b8fa5d0))
- a script's scroll write stays through a wheel event ([85541ec](https://github.com/tom96da/incajs/commit/85541ec))

## [0.0.16] - 2026-10-13

### Added

- the CLI prints help, its version and a usage for each command ([169758d](https://github.com/tom96da/incajs/commit/169758d))
- `inca dev` warns that it ignores the source map options ([169758d](https://github.com/tom96da/incajs/commit/169758d))

### Changed

- when the CLI gets opposite options together, the negative one takes precedence ([169758d](https://github.com/tom96da/incajs/commit/169758d))
- CLI error messages are plain sentences ([b2a9869](https://github.com/tom96da/incajs/commit/b2a9869))
- event error messages are easier to understand ([2cde8da](https://github.com/tom96da/incajs/commit/2cde8da))
- `activeElement()` and the element types moved from `incajs/vue` to `incajs` ([2048804](https://github.com/tom96da/incajs/commit/2048804)) ([f2f54b1](https://github.com/tom96da/incajs/commit/f2f54b1))
- `IncaCore` became an internal type ([50fc3ad](https://github.com/tom96da/incajs/commit/50fc3ad))

### Fixed

- with experimental HMR, a retried load after an error keeps window listeners on their own handlers ([5cc65ad](https://github.com/tom96da/incajs/commit/5cc65ad))
- an error stack maps to the original source when a function name contains a parenthesis ([b8db569](https://github.com/tom96da/incajs/commit/b8db569))
- on macOS the development window delivers its last reply to the CLI before it quits ([c43bd85](https://github.com/tom96da/incajs/commit/c43bd85))

## [0.0.15] - 2026-10-12

### Added

- mouse and wheel events carry `offsetX` and `offsetY` ([4b2825d](https://github.com/tom96da/incajs/commit/4b2825d))
- a template `ref` reads its box with `getBoundingClientRect()`, `offset*` and `client*` ([ab57a35](https://github.com/tom96da/incajs/commit/ab57a35))
- a template `ref` scrolls with `scrollTop`, `scrollLeft`, `scrollTo()` and `scrollBy()` ([10bb7b7](https://github.com/tom96da/incajs/commit/10bb7b7))
- the `scroll` event fires when a container scrolls ([be6ecde](https://github.com/tom96da/incajs/commit/be6ecde))
- `focus()` and Tab scroll the node into view, and a ref has `scrollIntoView()` ([22e125e](https://github.com/tom96da/incajs/commit/22e125e))
- `activeElement()` from `incajs/vue` returns the focused element ([42f3df0](https://github.com/tom96da/incajs/commit/42f3df0)) ([dea95b7](https://github.com/tom96da/incajs/commit/dea95b7))
- errors name the `.vue` or `.ts` file, line and column ([759a994](https://github.com/tom96da/incajs/commit/759a994))
- `inca build` and `inca package` write a source map with `sourcemap` or `--sourcemap` ([aeaa9af](https://github.com/tom96da/incajs/commit/aeaa9af))

### Fixed

- `el.tabIndex` reads the `tabindex` attribute of the template ([81ce80f](https://github.com/tom96da/incajs/commit/81ce80f))
- a ref of a removed element accepts listener and `tabIndex` calls ([81ce80f](https://github.com/tom96da/incajs/commit/81ce80f))

## [0.0.14] - 2026-10-11

### Added

- the `.capture` and `.passive` modifiers register capture and passive listeners ([966f0b8](https://github.com/tom96da/incajs/commit/966f0b8))
- `Event`, `CustomEvent`, `dispatchEvent()` and a ref's `click()` fire events from a script ([54a3089](https://github.com/tom96da/incajs/commit/54a3089))
- `window` from `incajs` listens at the root, and a ref has the three listener methods ([17aad54](https://github.com/tom96da/incajs/commit/17aad54))

### Changed

- the stack frames of a module under `--experimental-hmr` name the module ([2d893a8](https://github.com/tom96da/incajs/commit/2d893a8)) ([e562275](https://github.com/tom96da/incajs/commit/e562275))

### Fixed

- `mousedown`, `mouseup` and `click` fire in the order a pointer produces them ([68e79bd](https://github.com/tom96da/incajs/commit/68e79bd))
- `click` fires on the common ancestor of a press and the release of the same button ([68e79bd](https://github.com/tom96da/incajs/commit/68e79bd))
- `click` and `dblclick` skip a disabled `button` ([68e79bd](https://github.com/tom96da/incajs/commit/68e79bd))
- a click after a drag has `detail` 1 on macOS ([5121705](https://github.com/tom96da/incajs/commit/5121705))
- one event object reaches every listener on the path, so a stop or a property holds for the rest ([67b0af3](https://github.com/tom96da/incajs/commit/67b0af3))
- a canceled `mousedown` keeps the focus and a canceled key counts as handled ([67b0af3](https://github.com/tom96da/incajs/commit/67b0af3))
- `buttons` on a move after a release lists only the buttons still held ([67b0af3](https://github.com/tom96da/incajs/commit/67b0af3))
- template event names follow Vue: `@my-event` listens for `my-event` ([b28a142](https://github.com/tom96da/incajs/commit/b28a142))

## [0.0.13] - 2026-10-10

### Added

- style keys allow camelCase, kebab-case and snake_case ([acb8bbf](https://github.com/tom96da/incajs/commit/acb8bbf)) ([ac38fea](https://github.com/tom96da/incajs/commit/ac38fea))

### Fixed

- the development window opens at the app's content size under `--experimental-hmr` ([f73df37](https://github.com/tom96da/incajs/commit/f73df37)) ([3501563](https://github.com/tom96da/incajs/commit/3501563))
- `--experimental-hmr` recovers when the app starts on a broken file ([dfc20f6](https://github.com/tom96da/incajs/commit/dfc20f6))
- the shell's arrow keys keep working after Ctrl+C under `pnpm run` ([39c745a](https://github.com/tom96da/incajs/commit/39c745a))

## [0.0.12] - 2026-10-08

### Added

- events carry the standard base fields, and the event object has `composedPath()` ([e398454](https://github.com/tom96da/incajs/commit/e398454)) ([e880159](https://github.com/tom96da/incajs/commit/e880159))
- `button` element: default `tabindex` 0, `disabled`, and `click` from Enter and Space ([79556f0](https://github.com/tom96da/incajs/commit/79556f0))
- `tabindex` makes a node focusable, `el.tabIndex` sets it, and Tab and Shift+Tab move focus ([9f63f4a](https://github.com/tom96da/incajs/commit/9f63f4a)) ([3e946fa](https://github.com/tom96da/incajs/commit/3e946fa))
- `hidden` and `inert` attributes: `hidden` is `display: none`, `inert` ignores mouse and wheel ([02fa193](https://github.com/tom96da/incajs/commit/02fa193)) ([9fd5b5e](https://github.com/tom96da/incajs/commit/9fd5b5e))
- a focused node that becomes `hidden`, `inert` or `display: none` loses focus ([02fa193](https://github.com/tom96da/incajs/commit/02fa193))
- `focusin` and `focusout` events, and `relatedTarget` on the four focus events ([b4a6d39](https://github.com/tom96da/incajs/commit/b4a6d39))
- the focused node gets `blur` and `focusout` when the window loses focus, and `focus` on return ([9fd5b5e](https://github.com/tom96da/incajs/commit/9fd5b5e)) ([5a5c638](https://github.com/tom96da/incajs/commit/5a5c638))
- `keydown` and `keyup` carry `location`, `isComposing` and `getModifierState()` ([fab78ed](https://github.com/tom96da/incajs/commit/fab78ed))
- pressing or releasing `Shift`, `Control`, `Alt` or `Meta` alone fires `keydown` or `keyup` ([9fd5b5e](https://github.com/tom96da/incajs/commit/9fd5b5e))
- `click` carries `PointerEvent` fields, so `@click.ctrl`, `@click.left` and `.exact` work ([f8f4174](https://github.com/tom96da/incajs/commit/f8f4174))
- `dblclick`, `auxclick`, `contextmenu`, `mouseover` and `mouseout` events ([f8f4174](https://github.com/tom96da/incajs/commit/f8f4174)) ([849cf76](https://github.com/tom96da/incajs/commit/849cf76))
- mouse events carry `x`, `y`, `screenX`, `screenY`, `relatedTarget` and `getModifierState()` ([f8f4174](https://github.com/tom96da/incajs/commit/f8f4174)) ([849cf76](https://github.com/tom96da/incajs/commit/849cf76)) ([9fd5b5e](https://github.com/tom96da/incajs/commit/9fd5b5e))

### Changed

- `event.target` is the node the event started on, so `.self` and delegation work ([e880159](https://github.com/tom96da/incajs/commit/e880159))
- `keydown` and `keyup` fire on the root node when nothing is focused ([fab78ed](https://github.com/tom96da/incajs/commit/fab78ed))
- `key` is `"Unidentified"` when the platform reports none, and shifted letters keep their case ([fab78ed](https://github.com/tom96da/incajs/commit/fab78ed))
- `focus()` ignores nodes without `tabindex`, and a click outside every `tabindex` node blurs ([9f63f4a](https://github.com/tom96da/incajs/commit/9f63f4a))
- `mouseenter` and `mouseleave` fire on every entered or left ancestor ([849cf76](https://github.com/tom96da/incajs/commit/849cf76))
- `movementX` and `movementY` are set on `mousemove` only and are 0 on other events ([9fd5b5e](https://github.com/tom96da/incajs/commit/9fd5b5e))
- `console` prints a class instance with its class name ([d4a53bc](https://github.com/tom96da/incajs/commit/d4a53bc))
- `console` `%o` shows hidden keys and nests to depth 4 ([d4a53bc](https://github.com/tom96da/incajs/commit/d4a53bc))

### Fixed

- with `--experimental-hmr`, the last of two saves made within 50 ms of each other is applied ([9df1cce](https://github.com/tom96da/incajs/commit/9df1cce))
- `inca dev` rebuilds once when an editor saves a file by replacing it ([d3a4b74](https://github.com/tom96da/incajs/commit/d3a4b74))
- hovered nodes stay hovered while a key is pressed ([ea1829a](https://github.com/tom96da/incajs/commit/ea1829a))

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

[Unreleased]: https://github.com/tom96da/incajs/compare/v0.0.21...HEAD
[0.0.21]: https://github.com/tom96da/incajs/releases/tag/v0.0.21
[0.0.20]: https://github.com/tom96da/incajs/releases/tag/v0.0.20
[0.0.19]: https://github.com/tom96da/incajs/releases/tag/v0.0.19
[0.0.18]: https://github.com/tom96da/incajs/releases/tag/v0.0.18
[0.0.17]: https://github.com/tom96da/incajs/releases/tag/v0.0.17
[0.0.16]: https://github.com/tom96da/incajs/releases/tag/v0.0.16
[0.0.15]: https://github.com/tom96da/incajs/releases/tag/v0.0.15
[0.0.14]: https://github.com/tom96da/incajs/releases/tag/v0.0.14
[0.0.13]: https://github.com/tom96da/incajs/releases/tag/v0.0.13
[0.0.12]: https://github.com/tom96da/incajs/releases/tag/v0.0.12
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
