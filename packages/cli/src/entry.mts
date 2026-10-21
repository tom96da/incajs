// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import dedent from "ts-dedent";

import { IncaError } from "./error.mts";

export const MAIN_FILES = ["main.mts", "main.ts", "main.js"];

/**
 * The entry {@link resolveEntry} writes for an app that has only `src/App.vue`.
 * @param cwd - the app's root directory
 * @returns the path of the entry file
 */
function synthesizedEntryPath(cwd: string): string {
  return path.join(cwd, "node_modules/.inca/entry.mts");
}

/**
 * The directory Vite takes as the app's root: `src` for the entry
 * {@link resolveEntry} writes for `src/App.vue`, and the entry's own
 * directory for any other entry.
 * @param cwd - the app's root directory
 * @param entry - the app's entry file
 * @returns the directory
 */
export function entryRoot(cwd: string, entry: string): string {
  return entry === synthesizedEntryPath(cwd) ? path.join(cwd, "src") : path.dirname(entry);
}

/**
 * Resolves an app's entry point. A `src/main.mts`, `src/main.ts` or
 * `src/main.js` committed by the app wins outright, in that order, and is
 * used verbatim for full control over bootstrapping. Otherwise
 * `src/App.vue` is wrapped in a synthesized entry equivalent to
 * `createIncaApp(App).mount()`, so a plain `.vue` file is a whole app
 * with no configuration at all.
 *
 * @param cwd - the app's root directory
 * @returns the path of the entry file
 * @throws if none of the main files and no `src/App.vue` exists
 */
export async function resolveEntry(cwd: string): Promise<string> {
  const mainPaths = MAIN_FILES.map((file) => path.join(cwd, "src", file));
  const mainPath = mainPaths.find((candidate) => existsSync(candidate));
  if (mainPath) return mainPath;

  const appPath = path.join(cwd, "src/App.vue");
  if (existsSync(appPath)) return synthesizeEntry(appPath, cwd);

  throw new IncaError(
    "ERR_INCA_ENTRY_NOT_FOUND",
    `no app entry found. check that ${path.join(cwd, "src/main.ts")} or ${appPath} exists`,
  );
}

async function synthesizeEntry(appPath: string, cwd: string): Promise<string> {
  // Not under `outDir`: Vite refuses an entry whose directory is `outDir`
  // itself or a descendant of it, since a build's own root normally holds
  // its source rather than sitting inside what it writes.
  const entryPath = synthesizedEntryPath(cwd);
  await mkdir(path.dirname(entryPath), { recursive: true });

  const importSpecifier = JSON.stringify(appPath.split(path.sep).join("/"));
  await writeFile(
    entryPath,
    dedent`
      import { createIncaApp } from "incajs/vue";
      import App from ${importSpecifier};
      createIncaApp(App).mount();
    `,
  );
  return entryPath;
}
