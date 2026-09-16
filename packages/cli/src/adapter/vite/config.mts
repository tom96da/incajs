// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import vue from "@vitejs/plugin-vue";
import type { InlineConfig } from "vite";

import { captureOutput } from "./captureOutput.mts";
import type { CapturedOutput } from "./captureOutput.mts";

export const BUNDLE_FILE_NAME = "bundle.js";

export interface ResolveConfigOptions {
  /** The app's own entry point — may import `.vue` files. */
  entry: string;
  /** Where the build's output is written. */
  outDir: string;
  /** `"development"` keeps `@vue/runtime-core`'s own warnings; `"production"` strips them. */
  mode: "development" | "production";
  /** Whether Vite should keep rebuilding on file changes. */
  watch: boolean;
  /** Called with what each successful (re)build wrote. */
  onOutput: (output: CapturedOutput) => void;
}

/**
 * Builds the Vite config shared by `watch` and `build`, compiling `.vue`
 * files via `@vitejs/plugin-vue` targeted at `@vue/runtime-core` rather than
 * the `vue` meta-package it requires to run.
 *
 * An import the entry doesn't inline — a dynamic `import()`, or a `.vue`
 * file's own `<style>` block — lands as its own chunk or asset alongside
 * the entry, named by `chunkFileNames`/`assetFileNames` below, rather than
 * failing the build.
 */
export function resolveViteConfig({
  entry,
  outDir,
  mode,
  watch,
  onOutput,
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
      captureOutput(onOutput),
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
      rolldownOptions: {
        output: {
          chunkFileNames: "chunks/[name]-[hash].js",
          assetFileNames: "assets/[name]-[hash][extname]",
        },
      },
    },
  };
}
