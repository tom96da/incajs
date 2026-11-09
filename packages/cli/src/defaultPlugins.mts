// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import vue from "@vitejs/plugin-vue";
import dedent from "ts-dedent";

import * as adapterCore from "./adapterCore.mts";
import { createViteBundler } from "./vite/index.mts";
import type { Bundler, Frontend } from "./types.mts";
import type { ViteIntegration } from "./vite/index.mts";

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
 * The Vue plugin of Vite and the module that applies the stylesheets of a build.
 * @param runtimeModuleName - the module that compiled templates import their helpers from
 * @returns the integration the Vite adapter takes
 */
export function vueViteIntegration(runtimeModuleName: string): ViteIntegration {
  return {
    plugins: () => [
      vue({
        template: {
          // Hoisted static content becomes createStaticVNode, which needs insertStaticContent.
          compilerOptions: { runtimeModuleName, hoistStatic: false },
        },
      }),
    ],
    unsupported: [],
    inject: { document: ["incajs", "document"] },
    styleModule: "incajs/style",
  };
}

/**
 * The default stack: the frontend and the bundler `dev` and `build` share by default, a single
 * place to swap them. Compiled `.vue` templates import their helpers from `incajs/vue/runtime`.
 */
export const defaultBundler: Bundler = createViteBundler(
  adapterCore,
  defaultFrontend.id,
  vueViteIntegration("incajs/vue/runtime"),
);
