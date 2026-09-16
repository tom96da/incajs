// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { build as buildOnce } from "vite";

import { resolveViteConfig } from "./config.mts";
import type { BuildOptions, BuildOutput } from "../types.mts";

export type { BuildOptions, BuildOutput } from "../types.mts";

/**
 * Builds `entry` into a minified, production build under `outDir` once, and
 * rejects on failure rather than reporting it through a callback. Never
 * starts or talks to `inca-host` — that's `dev.mts`'s job.
 */
export async function build({ entry, outDir }: BuildOptions): Promise<BuildOutput> {
  let output: BuildOutput | undefined;
  await buildOnce(
    resolveViteConfig({
      entry,
      outDir,
      mode: "production",
      watch: false,
      onOutput: (captured) => {
        output = {
          outDir,
          entryFile: path.join(outDir, captured.entryFile),
          files: captured.files,
        };
      },
    }),
  );
  if (!output) throw new Error("inca: build produced no output");
  return output;
}
