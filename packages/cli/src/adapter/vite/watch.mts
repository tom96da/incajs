// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { build } from "vite";
import type { RolldownWatcher } from "rolldown";

import { resolveViteConfig } from "./config.mts";
import type { BundlerOptions, Watcher } from "../types.mts";

export type { Watcher } from "../types.mts";

/**
 * Builds `entry` into a build under `outDir` and rebuilds it on every
 * change, calling `onBuild` with what each rebuild wrote. Never starts,
 * reloads, or talks to `inca-host` — that's `dev.mts`'s job.
 */
export async function watch({
  onBuild,
  onError,
  stdout = process.stdout,
  stderr = process.stderr,
  quiet = false,
  ...buildOptions
}: BundlerOptions): Promise<Watcher> {
  const { outDir } = buildOptions;
  const result = await build(
    resolveViteConfig({
      ...buildOptions,
      watch: true,
      stdout,
      stderr,
      quiet,
      onOutput: (captured) => {
        onBuild({
          outDir,
          entryFile: path.join(outDir, captured.entryFile),
          files: captured.files,
        });
      },
    }),
  );

  // build() types its return as the non-watch output too, since a single
  // call signature covers both — watch: {} in the resolved config means
  // it's always this.
  const watcher = result as RolldownWatcher;

  watcher.on("event", (event) => {
    if (event.code === "ERROR") {
      onError({ message: event.error.message, stack: event.error.stack ?? null });
    }
  });

  return { close: () => watcher.close() };
}
