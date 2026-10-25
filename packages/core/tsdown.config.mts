// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    index: "src/index.mts",
    vue: "src/vue/index.mts",
    "vue-runtime": "src/vue/runtime.mts",
    // Not in package.json's `exports`. For inca-tests only.
    rendererCore: "src/rendererCore.mts",
  },
  format: "esm",
  fixedExtension: false,
  tsconfig: "tsconfig.runtime.json",
  deps: { neverBundle: ["@vue/runtime-core", "@vue/runtime-dom"] },
  dts: { generator: "tsgo", tsgo: { moduleUrl: import.meta.resolve("@typescript/native") } },
  clean: true,
  minify: true,
  outputOptions: {
    chunkFileNames: "chunks/[name]-[hash].js",
    codeSplitting: {
      groups: [
        { name: "hooks", test: /packages\/core\/src\/(callbacks|targets|interfaces)\.mts$/ },
      ],
    },
  },
});
