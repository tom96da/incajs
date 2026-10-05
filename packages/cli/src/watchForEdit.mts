// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync, watch } from "node:fs";
import path from "node:path";
import type { FSWatcher } from "node:fs";

import { MAIN_FILES } from "./entry.mts";

/** A watch that resolves once, when a file that could fix a failed start is saved. */
export interface EditWatch {
  /** Resolves once, after a relevant change has been quiet for the debounce. */
  readonly edited: Promise<void>;
  /** Also watches `file`, which may be missing. A change to that file counts. */
  add(file: string): void;
  /** Stops every watcher and timer. Safe to call more than once. */
  close(): void;
}

/** The files in the app's root directory that configure it. */
const CONFIG_FILE = /^(?:inca\.config\.[a-z0-9]+|package\.json)$/;

/** The entry files `resolveEntry` looks for in `src/`, by name. */
const ENTRY_FILES: readonly string[] = [...MAIN_FILES, "App.vue"];

/**
 * Watches the files that decide whether an app can start: `inca.config.*`
 * and `package.json` in `cwd`, `src/main.mts`, `src/main.ts`, `src/main.js` and `src/App.vue`, and every
 * file passed to {@link EditWatch.add}. Creating, deleting, changing or
 * renaming one of them counts as an edit. Edits to other files are ignored.
 *
 * The watchers are live when this returns, so a save made while the caller
 * is working counts. An edit made before the caller awaits
 * `edited` keeps it resolved.
 *
 * @param cwd - the app's root directory
 * @param options.debounceMs - how long changes must stay quiet before `edited` resolves. Defaults to 100.
 * @returns the watch. Call `close()` when done with it.
 * @throws if the operating system refuses a watcher, for example when it is out of file handles
 */
export function watchForEdit(cwd: string, options: { debounceMs?: number } = {}): EditWatch {
  const debounceMs = options.debounceMs ?? 100;
  const watchers = new Set<FSWatcher>();
  let timer: NodeJS.Timeout | undefined;
  let closed = false;
  let fire = (): void => {};
  const edited = new Promise<void>((resolve) => {
    fire = resolve;
  });

  const touch = (): void => {
    if (closed) return;
    clearTimeout(timer);
    timer = setTimeout(fire, debounceMs);
    timer.unref();
  };

  // A directory that is missing is watched through its nearest existing
  // ancestor, where its own creation shows up as the next path segment.
  const watchDir = (dir: string, wanted: (name: string) => boolean): void => {
    let base = dir;
    for (let parent; !existsSync(base) && (parent = path.dirname(base)) !== base;) base = parent;
    const next = base === dir ? undefined : path.relative(base, dir).split(path.sep)[0];

    const watcher = watch(base);
    watcher.on("change", (_event, changed: string | Buffer | null) => {
      const name = changed === null ? null : String(changed);
      if (name === null || (next === undefined ? wanted(name) : name === next)) touch();
    });
    // A dead watcher counts as an edit, so the caller retries with a fresh one.
    watcher.on("error", () => {
      watcher.close();
      watchers.delete(watcher);
      touch();
    });
    watchers.add(watcher);
  };

  const close = (): void => {
    closed = true;
    clearTimeout(timer);
    for (const watcher of watchers) watcher.close();
    watchers.clear();
  };

  try {
    // `src` counts so that a deleted and recreated `src/` re-arms the caller.
    watchDir(cwd, (name) => name === "src" || CONFIG_FILE.test(name));
    watchDir(path.join(cwd, "src"), (name) => ENTRY_FILES.includes(name));
  } catch (error) {
    close();
    throw error;
  }

  return {
    edited,
    add(file) {
      if (closed) return;
      const resolved = path.resolve(file);
      watchDir(path.dirname(resolved), (name) => name === path.basename(resolved));
    },
    close,
  };
}
