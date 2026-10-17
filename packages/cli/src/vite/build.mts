// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { build as buildOnce } from "vite";

import { toBuildOutput } from "./captureOutput.mts";
import { resolveViteConfig } from "./config.mts";
import type { AdapterCore, BuildOptions, BuildOutput } from "../adapterCore.mts";

/**
 * Builds `entry` into a minified, production build under `outDir` once, and
 * rejects on failure. Never starts or talks to `inca-host`.
 */
export async function build(
  core: AdapterCore,
  runtimeModuleName: string,
  {
    entry,
    outDir,
    sourcemap,
    runtimeConfig,
    stdout = process.stdout,
    stderr = process.stderr,
    quiet = false,
  }: BuildOptions,
): Promise<BuildOutput> {
  let output: BuildOutput | undefined;
  await buildOnce(
    resolveViteConfig({
      core,
      runtimeModuleName,
      entry,
      outDir,
      runtimeConfig,
      mode: "production",
      sourcemap,
      watch: false,
      stdout,
      stderr,
      quiet,
      onOutput: (captured) => {
        output = toBuildOutput(outDir, captured);
      },
    }),
  );
  if (!output) throw new core.IncaError("ERR_INCA_BUILD_NO_OUTPUT", "build produced no output");
  return output;
}
