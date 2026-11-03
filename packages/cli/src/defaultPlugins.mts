// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import dedent from "ts-dedent";

import * as adapterCore from "./adapterCore.mts";
import { createViteBundler } from "./vite/index.mts";
import type { Bundler, Frontend } from "./types.mts";

/** The Vue frontend: `src/App.vue` is a whole app. */
export const defaultFrontend: Frontend = {
  id: ["incajs", "vue"],
  appFile: "App.vue",
  entryCode: (appImport) => dedent`
    import { createIncaApp } from "incajs/vue";
    import App from ${appImport};
    createIncaApp(App).mount();
  `,
};

/**
 * `vite/` wired in as the one adapter `dev` and `build` share by
 * default: a single place to swap bundlers. Compiled `.vue` templates import
 * their helpers from `incajs/vue/runtime`.
 */
export const defaultBundler: Bundler = createViteBundler(adapterCore, defaultFrontend.id, {
  runtimeModuleName: "incajs/vue/runtime",
});
