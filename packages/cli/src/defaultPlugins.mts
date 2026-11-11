// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import vue from "@vitejs/plugin-vue";
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
 * The default stack: the frontend and the bundler `dev` and `build` share by default, a single
 * place to swap them.
 */
export const defaultBundler: Bundler = createViteBundler(adapterCore, {
  frontend: defaultFrontend.id,
  plugins: () => [
    vue({
      template: {
        // Hoisted static content becomes createStaticVNode, which needs insertStaticContent.
        compilerOptions: { hoistStatic: false },
      },
    }),
  ],
  unsupported: [],
  inject: { document: ["incajs", "document"] },
  styleModule: "incajs/style",
});
