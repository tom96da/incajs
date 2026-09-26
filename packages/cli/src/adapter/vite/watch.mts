// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { build } from "vite";
import type { RolldownWatcher } from "rolldown";

import { bundlerFault } from "../../log.mts";
import { toBuildOutput } from "./captureOutput.mts";
import { resolveViteConfig } from "./config.mts";
import type { BundlerOptions, Watcher } from "../types.mts";

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
  let changed: { file: string; at: number } | undefined;
  const result = await build(
    resolveViteConfig({
      ...buildOptions,
      watch: true,
      stdout,
      stderr,
      quiet,
      onOutput: (captured) => {
        onBuild(toBuildOutput(outDir, captured, changed));
        changed = undefined;
      },
    }),
  );

  // build() types its return as the non-watch output too, since a single
  // call signature covers both — watch: {} in the resolved config means
  // it's always this.
  const watcher = result as RolldownWatcher;

  watcher.on("change", (id) => {
    changed ??= { file: id, at: Date.now() };
  });

  watcher.on("event", (event) => {
    if (event.code !== "ERROR") return;
    // Nothing to reload for, so the next successful build times from the
    // edit that fixed this one.
    changed = undefined;
    // Rolldown aggregates: errors[0] is the one a plugin actually raised.
    const { errors } = event.error as { errors?: { message?: string }[] };
    onError(bundlerFault(errors?.[0]?.message ?? event.error.message));
  });

  return { close: () => watcher.close() };
}
