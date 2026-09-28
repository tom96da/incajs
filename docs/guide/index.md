---
description: Get up and running with Incarnative.js. Learn how to install, write, and run your app.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Getting Started

For how the pieces fit together, see [How it Works](./how-it-works).

## Installation

[Node.js](https://nodejs.org/) v22.18 or higher.

::: code-group

```sh [npm]
$ npm install incajs @vue/runtime-core
$ npm install -D @incajs/cli
```

```sh [pnpm]
$ pnpm add incajs @vue/runtime-core
$ pnpm add -D @incajs/cli
```

```sh [yarn]
$ yarn add incajs @vue/runtime-core
$ yarn add -D @incajs/cli
```

:::

> [!WARNING]
> Incarnative.js supports only ES modules.

## File Structure

Scaffold your app in its own directory (e.g. `./my-app`), with a
`package.json` and a `src` directory:

```
.
├─ src
│  └─ App.vue
├─ inca.config.ts
└─ package.json
```

The directory containing `package.json` is the app's **project
root** — every path `inca` resolves, including `src` and the `dist`
it writes, is relative to it.

`package.json` needs `"type": "module"`, satisfying the ESM-only
requirement above:

```json [package.json]
{
  "type": "module"
}
```

> [!TIP]
> `inca build`/`package` write their output to `dist`. If using Git,
> add it to your `.gitignore` file.

### Source Files

Files under `src` are your app's **source files**. `inca` looks for
exactly one of two entry points there:

- `src/App.vue` — no entry point or bootstrapping code needed; `inca`
  mounts it as the app itself:

  ```vue [src/App.vue]
  <script setup>
  import { ref } from "@vue/runtime-core";

  const clicks = ref(0);
  </script>

  <template>
    <div :style="{ text_color: 0xffffff }" @click="clicks++">
      Clicked {{ clicks }} time(s)
    </div>
  </template>
  ```

  See [Elements & Styles](../reference/elements) for every style key and
  what's supported.

- `src/main.mts` — for full control over bootstrapping. It wins
  outright when both exist.

## Up and Running

Add an npm script like the following to `package.json`.

```json [package.json]
{
  ...
  "scripts": {
    "dev": "inca dev",          // [!code ++]
    "build": "inca build",      // [!code ++]
    "package": "inca package"   // [!code ++]
  },
  ...
}
```

The `dev` script watches your app and opens a live-reloading window.

::: code-group

```sh [npm]
$ npm run dev
```

```sh [pnpm]
$ pnpm run dev
```

```sh [yarn]
$ yarn run dev
```

:::


## Host binary

A prebuilt binary is pulled in automatically as a platform-specific
optional dependency. Neither Cargo nor Rust are needed to run an app.

| Platform      | Status           |
| ------------- | ---------------- |
| Linux x64     | Limited support* |
| Linux arm64   | Limited support* |
| macOS arm64   | Supported        |
| macOS x64     | Not planned      |
| Windows x64   | Planned          |
| Windows arm64 | Planned          |

\* The app still needs [these libraries](#linux-runtime-requirements) on
the machine that runs it, whether via the development window or a
packaged app.
`inca package` doesn't bundle them yet.

## Linux runtime requirements

The host binary is built against glibc 2.35, so it needs Ubuntu 22.04,
Debian 12, or anything newer.

On Debian/Ubuntu, it also needs:

- `libxcb1`
- `libfontconfig1`
- `libfreetype6`
- `libxkbcommon0`
- `libxkbcommon-x11-0`
- `libvulkan1`
- a Vulkan driver package, such as `mesa-vulkan-drivers`

Other distributions use the same libraries under different package
names. They're present by default on most desktops, but a minimal or
headless install may need them added explicitly.

Some of these are already unnecessary. The binary bundles FreeType, and
`libfontconfig1` is linked on arm64 only. The list will be narrowed once
it is measured on a minimal install.

The Vulkan loader and driver are dlopen'd at startup rather than
linked, so `ldd` won't show them. Rendering can't start without them.
