---
description: Hot module replacement under inca dev --experimental-hmr — how to enable it and what it does differently.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Hot Module Replacement <Badge type="warning" text="Experimental"/>

> [!WARNING]
> HMR is experimental and opt-in. Plain `inca dev` (no flag) always does
> a full reload.

## Enabling it

```sh
inca dev --experimental-hmr
```

or:

```sh
INCA_EXPERIMENTAL_HMR=1 inca dev
```

## What it does differently

A full reload re-evaluates the whole bundle and remounts the app from
scratch. With experimental HMR enabled, `inca dev` instead reloads only
the changed module:

- A template-only edit preserves the component's state.
- A script edit resets state — the component remounts.
