// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import vue from "@vitejs/plugin-vue";
import type { InlineConfig } from "vite";

export const BUNDLE_FILE_NAME = "bundle.js";

export interface ResolveConfigOptions {
  /** The app's own entry point — may import `.vue` files. */
  entry: string;
  /** Where the self-contained bundle is written. */
  outDir: string;
  /** `"development"` keeps `@vue/runtime-core`'s own warnings; `"production"` strips them. */
  mode: "development" | "production";
  /** Whether Vite should keep rebuilding on file changes. */
  watch: boolean;
}

/**
 * Builds the Vite config shared by `watch` and `build`, compiling `.vue`
 * files via `@vitejs/plugin-vue` targeted at `@vue/runtime-core` rather than
 * the `vue` meta-package it requires to run.
 */
export function resolveViteConfig({
  entry,
  outDir,
  mode,
  watch,
}: ResolveConfigOptions): InlineConfig {
  return {
    configFile: false,
    root: path.dirname(entry),
    mode,
    clearScreen: false,
    logLevel: "silent",
    define: {
      "process.env.NODE_ENV": JSON.stringify(mode),
    },
    plugins: [
      vue({
        template: {
          compilerOptions: { runtimeModuleName: "@vue/runtime-core" },
        },
      }),
    ],
    build: {
      lib: {
        entry,
        formats: ["es"],
        fileName: () => BUNDLE_FILE_NAME,
      },
      outDir,
      // Explicit either way: silences Vite's own notice about defaulting to
      // false here, since outDir sits outside root in this project's layout
      // regardless of mode.
      emptyOutDir: mode === "production",
      minify: mode === "production",
      watch: watch ? {} : undefined,
    },
  };
}
