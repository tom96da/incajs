// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { build as buildOnce } from "vite";

import { toBuildOutput } from "./captureOutput.mts";
import { resolveViteConfig } from "./config.mts";
import type { AdapterCore } from "../../adapterCore.mts";
import type { BuildOptions, BuildOutput } from "../types.mts";

/**
 * Builds `entry` into a minified, production build under `outDir` once, and
 * rejects on failure rather than reporting it through a callback. Never
 * starts or talks to `inca-host` — that's `dev.mts`'s job.
 */
export async function build(
  core: AdapterCore,
  {
    entry,
    outDir,
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
      entry,
      outDir,
      runtimeConfig,
      mode: "production",
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
