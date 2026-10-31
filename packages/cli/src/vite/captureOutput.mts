// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readdir, stat } from "node:fs/promises";
import path from "node:path";

import type { Plugin } from "vite";

import type { AdapterCore, BuildOutput } from "../adapterCore.mts";

/** What one (re)build actually wrote, relative to its own `outDir`. */
export interface CapturedOutput {
  /** The emitted chunk marked as this build's entry. */
  entryFile: string;
  /** Every emitted file, as the bundler reported it. */
  files: readonly string[];
}

/** Resolves `captured`'s paths against `outDir` into a {@link BuildOutput}. */
export function toBuildOutput(
  outDir: string,
  captured: CapturedOutput,
  changed?: BuildOutput["changed"],
): BuildOutput {
  return {
    outDir,
    entryFile: path.join(outDir, captured.entryFile),
    files: captured.files,
    changed,
  };
}

/**
 * Lists the files under `dir`, following symlinks. An unreadable directory contributes none and an
 * entry that cannot be inspected counts as absent.
 *
 * @param dir - the directory to walk
 * @param prefix - the `/`-separated path of `dir` below the starting directory
 * @returns the paths relative to the starting directory, `/`-separated
 */
async function listFiles(dir: string, prefix = ""): Promise<string[]> {
  let names: string[];
  try {
    names = await readdir(dir);
  } catch {
    return [];
  }
  const nested = await Promise.all(
    names.map(async (name) => {
      const info = await stat(path.join(dir, name)).catch(() => undefined);
      if (!info) return [];
      return info.isDirectory()
        ? listFiles(path.join(dir, name), `${prefix}${name}/`)
        : [prefix + name];
    }),
  );
  return nested.flat();
}

/**
 * A Vite plugin reporting `sink` the files a build wrote once they land on
 * disk, via `writeBundle`, the one hook that fires uniformly for a
 * one-shot build and every watch rebuild. The files Vite copied from the
 * public directory count as written.
 */
export function captureOutput(core: AdapterCore, sink: (output: CapturedOutput) => void): Plugin {
  // Empty when public files are not copied; the walk of "" fails and lists nothing
  let publicDir = "";
  return {
    name: "inca:capture-output",
    configResolved(config) {
      publicDir = config.build.copyPublicDir ? config.publicDir : "";
    },
    async writeBundle(_options, bundle) {
      const files = [...new Set([...Object.keys(bundle), ...(await listFiles(publicDir))])];
      const entry = Object.values(bundle).find((file) => file.type === "chunk" && file.isEntry);
      if (!entry) {
        const code = "ERR_INCA_BUILD_NO_ENTRY_CHUNK";
        throw new core.IncaError(code, `${code}: build produced no entry chunk`);
      }
      sink({ entryFile: entry.fileName, files });
    },
  };
}
