// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { build } from "vite";
import type { RolldownWatcher } from "rolldown";

import { BUNDLE_FILE_NAME, resolveViteConfig } from "./config.mts";
import type { BundlerOptions, Watcher } from "../types.mts";

export type { Watcher } from "../types.mts";

/**
 * Builds `entry` into a bundle under `outDir` and rebuilds it on every
 * change. Never starts, reloads, or talks to `inca-host` — that's
 * `dev.mts`'s job.
 */
export async function watch({
  onBuild,
  onError,
  ...buildOptions
}: BundlerOptions): Promise<Watcher> {
  const bundlePath = path.join(buildOptions.outDir, BUNDLE_FILE_NAME);
  const result = await build(resolveViteConfig({ ...buildOptions, watch: true }));

  // build() types its return as the non-watch output too, since a single
  // call signature covers both — watch: {} in the resolved config means
  // it's always this.
  const watcher = result as RolldownWatcher;

  watcher.on("event", (event) => {
    if (event.code === "END") {
      onBuild(bundlePath);
    } else if (event.code === "ERROR") {
      onError({ message: event.error.message, stack: event.error.stack ?? null });
    }
  });

  return { bundlePath, close: () => watcher.close() };
}
