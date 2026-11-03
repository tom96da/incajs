// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { IncaError } from "./error.mts";
import type { Frontend } from "./types.mts";

export const MAIN_FILES = ["main.mts", "main.ts", "main.js"];

/**
 * The entry {@link resolveEntry} writes for an app that has only the app file of the frontend.
 * @param cwd - the app's root directory
 * @returns the path of the entry file
 */
function synthesizedEntryPath(cwd: string): string {
  return path.join(cwd, "node_modules/.inca/entry.mts");
}

/**
 * The directory Vite takes as the app's root: `src` for the entry
 * {@link resolveEntry} writes for the app file of the frontend, and the entry's own
 * directory for any other entry.
 * @param cwd - the app's root directory
 * @param entry - the app's entry file
 * @returns the directory
 */
export function entryRoot(cwd: string, entry: string): string {
  return entry === synthesizedEntryPath(cwd) ? path.join(cwd, "src") : path.dirname(entry);
}

/**
 * The path {@link resolveEntry} would return, without writing the synthesized entry.
 * @param cwd - the app's root directory
 * @param frontend - the frontend whose app file may stand for a whole app
 * @returns the path of the entry file, or `undefined` when no entry exists
 */
export function selectEntry(cwd: string, frontend: Frontend): string | undefined {
  const mainPaths = MAIN_FILES.map((file) => path.join(cwd, "src", file));
  const mainPath = mainPaths.find((candidate) => existsSync(candidate));
  if (mainPath) return mainPath;

  return existsSync(path.join(cwd, "src", frontend.appFile))
    ? synthesizedEntryPath(cwd)
    : undefined;
}

/**
 * Resolves an app's entry point. A `src/main.mts`, `src/main.ts` or
 * `src/main.js` committed by the app wins outright, in that order, and is
 * used verbatim for full control over bootstrapping. Otherwise the app file
 * of the frontend is wrapped in a synthesized entry, so that file alone is a
 * whole app with no configuration at all.
 *
 * @param cwd - the app's root directory
 * @param frontend - the frontend whose app file may stand for a whole app
 * @returns the path of the entry file
 * @throws if none of the main files and no app file of the frontend exists
 */
export async function resolveEntry(cwd: string, frontend: Frontend): Promise<string> {
  const appPath = path.join(cwd, "src", frontend.appFile);
  const selected = selectEntry(cwd, frontend);
  if (selected === synthesizedEntryPath(cwd)) return synthesizeEntry(appPath, cwd, frontend);
  if (selected) return selected;

  throw new IncaError(
    "ERR_INCA_ENTRY_NOT_FOUND",
    `no app entry found. check that ${path.join(cwd, "src/main.ts")} or ${appPath} exists`,
  );
}

async function synthesizeEntry(appPath: string, cwd: string, frontend: Frontend): Promise<string> {
  // Not under `outDir`: Vite refuses an entry whose directory is `outDir`
  // itself or a descendant of it, since a build's own root normally holds
  // its source rather than sitting inside what it writes.
  const entryPath = synthesizedEntryPath(cwd);
  await mkdir(path.dirname(entryPath), { recursive: true });

  const importSpecifier = JSON.stringify(appPath.split(path.sep).join("/"));
  await writeFile(entryPath, frontend.entryCode(importSpecifier));
  return entryPath;
}
