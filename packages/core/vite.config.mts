// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import dts from "unplugin-dts/vite";
import { defineConfig } from "vite";

export default defineConfig({
  build: {
    lib: {
      entry: {
        index: path.resolve(import.meta.dirname, "src/index.mts"),
        vue: path.resolve(import.meta.dirname, "src/vue/index.mts"),
      },
      formats: ["es"],
    },
    rolldownOptions: {
      external: ["@vue/runtime-core"],
      output: { chunkFileNames: "chunks/[name]-[hash].js" },
    },
  },
  plugins: [dts({ include: ["src"], exclude: ["src/**/*.test.mts"] })],
});
