// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { defaultBundler } from "./defaultBundler.mts";
import { resolveEntry } from "./entry.mts";
import type { Bundler } from "./adapter/types.mts";

export interface BuildAppOptions {
  /** The app's root directory. Defaults to `process.cwd()`. */
  cwd?: string;
  /**
   * The app's entry point. Defaults to resolving it automatically: a
   * committed `src/main.mts`, or `src/App.vue` wrapped in a synthesized one.
   */
  entry?: string;
  /** Overrides the bundler — see {@link defaultBundler} for what's wired in by default. */
  bundler?: Bundler;
}

/**
 * Builds the app once through the same bundler `dev` uses, and returns the
 * bundle's path. Rejects on failure — deciding what that means for the
 * process is `cli.mts`'s job.
 */
export async function build(options: BuildAppOptions = {}): Promise<string> {
  const cwd = options.cwd ?? process.cwd();
  const outDir = path.join(cwd, "dist");
  const bundler: Bundler = options.bundler ?? defaultBundler;

  const entry = options.entry ?? (await resolveEntry(cwd));

  const { bundlePath } = await bundler.build({ entry, outDir });
  return bundlePath;
}
