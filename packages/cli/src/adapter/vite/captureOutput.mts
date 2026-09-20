// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Plugin } from "vite";

import { IncaError } from "../../error.mts";

/** What one (re)build actually wrote, relative to its own `outDir`. */
export interface CapturedOutput {
  /** The emitted chunk marked as this build's entry. */
  entryFile: string;
  /** Every emitted file, as the bundler reported it. */
  files: readonly string[];
}

/**
 * A Vite plugin reporting `sink` the files a build wrote once they land on
 * disk, via `writeBundle` — the one hook that fires uniformly for a
 * one-shot build and every watch rebuild, so both read the bundler's real
 * output instead of assuming a file name or layout.
 */
export function captureOutput(sink: (output: CapturedOutput) => void): Plugin {
  return {
    name: "inca:capture-output",
    writeBundle(_options, bundle) {
      const files = Object.keys(bundle);
      const entry = Object.values(bundle).find((file) => file.type === "chunk" && file.isEntry);
      if (!entry) {
        throw new IncaError("ERR_INCA_BUILD_NO_ENTRY_CHUNK", "build produced no entry chunk");
      }
      sink({ entryFile: entry.fileName, files });
    },
  };
}
