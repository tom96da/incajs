// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { resolveBuildConfig } from "./config/loader.mts";
import { defaultBundler } from "./defaultBundler.mts";
import { resolveEntry } from "./entry.mts";
import type { Bundler, BuildOutput } from "./adapter/types.mts";
import type { ResolvedBuildConfig } from "./config/loader.mts";

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
  /** An already-resolved config — lets `packageApp` avoid loading it twice. */
  config?: ResolvedBuildConfig;
  /** Where the bundler's own output goes. Defaults to `process.stdout`. */
  stdout?: NodeJS.WritableStream;
  /** Where the bundler's warnings and errors go. Defaults to `process.stderr`. */
  stderr?: NodeJS.WritableStream;
}

/**
 * Builds the app once through the same bundler `dev` uses, and returns what
 * it wrote. Rejects on failure — deciding what that means for the process
 * is `cli.mts`'s job.
 */
export async function build(options: BuildAppOptions = {}): Promise<BuildOutput> {
  const cwd = options.cwd ?? process.cwd();
  const config = options.config ?? (await resolveBuildConfig(cwd));
  const outDir = config.outDir;
  const bundler: Bundler = options.bundler ?? defaultBundler;

  const entry = options.entry ?? config.entry ?? (await resolveEntry(cwd));

  return bundler.build({ entry, outDir, stdout: options.stdout, stderr: options.stderr });
}
