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
        cli: path.resolve(import.meta.dirname, "src/cli.mts"),
      },
      formats: ["es"],
      fileName: (_format, entryName) => `${entryName}.js`,
    },
    rolldownOptions: {
      // This package only ever runs in Node — nothing here targets a
      // browser bundle, so every `node:` builtin stays an import rather
      // than something Vite tries to polyfill.
      external: [/^node:/, "vite", "@vitejs/plugin-vue"],
    },
  },
  plugins: [dts({ include: ["src"], exclude: ["src/**/*.test.mts"] })],
});
