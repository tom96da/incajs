// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import type { InlineConfig } from "vite";

import { captureOutput } from "./captureOutput.mts";
import { emitConfig } from "./emitConfig.mts";
import { integrationPlugins } from "./integration.mts";
import { streamLogger } from "./logger.mts";
import type { AdapterCore, FrontendId } from "../adapterCore.mts";
import type { CapturedOutput } from "./captureOutput.mts";
import type { ViteIntegration } from "./integration.mts";

export const BUNDLE_FILE_NAME = "bundle.js";

export interface ResolveConfigOptions {
  core: AdapterCore;
  /** The frontend to bundle. Its package lists the peers the app must install. */
  frontend: FrontendId;
  /** The frontend's Vite plugins, style module and refused features. */
  integration: ViteIntegration;
  /** The app's own entry point. It may import the files of the frontend. */
  entry: string;
  /** See the `root` option in `../types.mts`. Defaults to the entry's directory. */
  root?: string;
  /** See the `envDir` option in `../types.mts`. Defaults to `root`. */
  envDir?: string;
  /** See the `publicDir` option in `../types.mts`. Defaults to `public` under `root`. */
  publicDir?: string;
  /** Where the build's output is written. */
  outDir: string;
  /** `"development"` keeps the runtime's own warnings; `"production"` strips them. */
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
 * Builds the Vite config shared by `watch` and `build`, adding the plugins of the
 * frontend's integration.
 *
 * An import the entry doesn't inline (a dynamic `import()`) lands as its own chunk alongside
 * the entry, named by `chunkFileNames` below.
 */
export function resolveViteConfig({
  core,
  frontend,
  integration,
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
      ...integrationPlugins(core, frontend, integration, root),
      emitConfig(runtimeConfig),
      captureOutput(core, onOutput),
    ],
    build: {
      // Library mode takes a JS entry. App mode would need an HTML entry.
      lib: {
        entry,
        // Library mode names a css file even for empty css and throws when the package has no name.
        cssFileName: "inca-style",
        formats: ["es"],
        fileName: () => BUNDLE_FILE_NAME,
      },
      outDir,
      // Explicit either way: silences Vite's notice about defaulting to false here.
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
