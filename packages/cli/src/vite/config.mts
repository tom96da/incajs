// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import vue from "@vitejs/plugin-vue";
import type { Options as VueOptions } from "@vitejs/plugin-vue";
import type { InlineConfig } from "vite";

import { captureOutput } from "./captureOutput.mts";
import { emitConfig } from "./emitConfig.mts";
import { streamLogger } from "./logger.mts";
import { requirePeers } from "./requirePeers.mts";
import { rejectUnsupported } from "./unsupported.mts";
import type { AdapterCore } from "../adapterCore.mts";
import type { CapturedOutput } from "./captureOutput.mts";

export const BUNDLE_FILE_NAME = "bundle.js";

/** A frontend: the package that lists its peers, and its name there. */
export type FrontendId = readonly [pkg: string, frontend: string];

/** The options of the `vue` plugin's template compiler. */
type VueTemplateCompilerOptions = NonNullable<
  NonNullable<VueOptions["template"]>["compilerOptions"]
>;

export type FrontendOptions = VueTemplateCompilerOptions;

export interface ResolveConfigOptions {
  core: AdapterCore;
  /** The frontend to bundle. Its package lists the peers the app must install. */
  frontend: FrontendId;
  /** The frontend's own settings. */
  frontendOptions: FrontendOptions;
  /** The app's own entry point. It may import `.vue` files. */
  entry: string;
  /** See the `root` option in `../types.mts`. Defaults to the entry's directory. */
  root?: string;
  /** See the `envDir` option in `../types.mts`. Defaults to `root`. */
  envDir?: string;
  /** See the `publicDir` option in `../types.mts`. Defaults to `public` under `root`. */
  publicDir?: string;
  /** Where the build's output is written. */
  outDir: string;
  /** `"development"` keeps `@vue/runtime-core`'s own warnings; `"production"` strips them. */
  mode: "development" | "production";
  /** Writes the map in a production build too. Development always writes it. */
  sourcemap?: boolean;
  /** Whether Vite should keep rebuilding on file changes. */
  watch: boolean;
  /** Called with what each successful (re)build wrote. */
  onOutput: (output: CapturedOutput) => void;
  /** Where Vite's own build output goes. */
  stdout: NodeJS.WritableStream;
  /** Where Vite's warnings and errors go. */
  stderr: NodeJS.WritableStream;
  /** Silences the bundler's progress output. See the `quiet` option in `../types.mts`. */
  quiet: boolean;
  /** Written beside the entry. See the `runtimeConfig` option in `../types.mts`. */
  runtimeConfig?: unknown;
}

/**
 * Builds the Vite config shared by `watch` and `build`, compiling `.vue`
 * files via `@vitejs/plugin-vue`. Compiled templates import their helpers
 * from `options.runtimeModuleName`.
 *
 * An import the entry doesn't inline (a dynamic `import()`, or a `.vue`
 * file's own `<style>` block) lands as its own chunk or asset alongside
 * the entry, named by `chunkFileNames`/`assetFileNames` below.
 */
export function resolveViteConfig({
  core,
  frontend,
  frontendOptions,
  entry,
  root = path.dirname(entry),
  envDir,
  publicDir,
  outDir,
  mode,
  sourcemap = false,
  watch,
  onOutput,
  stdout,
  stderr,
  quiet,
  runtimeConfig,
}: ResolveConfigOptions): InlineConfig {
  const emitMap = mode === "development" || sourcemap;
  return {
    configFile: false,
    root,
    envDir,
    publicDir,
    mode,
    clearScreen: false,
    // The only knob rolldown's native reporter reads. Silencing it also
    // drops the size report.
    logLevel: quiet ? "silent" : "info",
    // Not `quiet`: this path's info() carries the per-build summary quiet
    // mode is supposed to keep. See `BuildOptions.quiet`.
    customLogger: streamLogger(stdout, stderr),
    define: {
      "process.env.NODE_ENV": JSON.stringify(mode),
    },
    plugins: [
      vue({
        template: {
          // Hoisted static content is stringified into createStaticVNode, which needs insertStaticContent.
          compilerOptions: { ...frontendOptions, hoistStatic: false },
        },
      }),
      requirePeers(core, frontend),
      rejectUnsupported(core),
      emitConfig(runtimeConfig),
      captureOutput(core, onOutput),
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
      sourcemap: emitMap,
      // An editor that replaces a file deletes it for a moment; the delay lets that pair coalesce.
      watch: watch ? { buildDelay: 30 } : undefined,
      rolldownOptions: {
        output: {
          chunkFileNames: "chunks/[name]-[hash].js",
          assetFileNames: "assets/[name]-[hash][extname]",
          // The map holds paths and positions only.
          sourcemapExcludeSources: emitMap,
        },
      },
    },
  };
}
