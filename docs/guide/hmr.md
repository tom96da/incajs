---
description: What hot module replacement does under inca dev and how to turn it off.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Hot Module Replacement

A full reload evaluates the whole bundle again and mounts the app from scratch. `inca dev` replaces
the module you edited when a component or a style accepts the edit:

- A template-only edit preserves the component's state.
- An edit of a `<style>` or `<style scoped>` block preserves the state of the
  components.
- A script edit resets state. The component remounts.
- A stack frame of a module names the module's source file, line and column.

## Turning it off

Set `dev.hmr` to `false` in `inca.config.ts` to reload the whole app on every edit:

```ts
import { defineConfig } from "@incajs/cli/config";

export default defineConfig({
  dev: { hmr: false },
});
```

See [the window guide](./window) for when `inca dev` reads the config.
