// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { build as buildOnce } from "vite";

import { BUNDLE_FILE_NAME, resolveViteConfig } from "./config.mts";
import type { BuildOptions, BuildResult } from "../types.mts";

export type { BuildOptions, BuildResult } from "../types.mts";

/**
 * Builds `entry` into a minified, production bundle under `outDir` once, and
 * rejects on failure rather than reporting it through a callback. Never
 * starts or talks to `inca-host` — that's `dev.mts`'s job.
 */
export async function build({ entry, outDir }: BuildOptions): Promise<BuildResult> {
  await buildOnce(resolveViteConfig({ entry, outDir, mode: "production", watch: false }));
  return { bundlePath: path.join(outDir, BUNDLE_FILE_NAME) };
}
