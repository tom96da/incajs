// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    index: "src/index.mts",
    cli: "src/cli.mts",
    config: "src/config/index.mts",
    "vite-runtime": "src/vite/runtime/index.mts",
  },
  format: "esm",
  fixedExtension: false,
  tsconfig: "tsconfig.node.json",
  platform: "node",
  deps: {
    neverBundle: [/^node:/, /^vite$/, "@vitejs/plugin-vue", "giget", "jiti"],
    alwaysBundle: ["vite/module-runner"],
  },
  dts: { generator: "tsgo", tsgo: { moduleUrl: import.meta.resolve("@typescript/native") } },
  clean: true,
  publint: true,
  attw: { level: "error" },
  minify: true,
  outputOptions: { chunkFileNames: "chunks/[name]-[hash].js" },
});
