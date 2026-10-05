// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { randomUUID } from "node:crypto";
import { link, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { IncaError } from "./error.mts";

/** Where a running `inca dev` records its pid, for the next one to find. */
function lockPath(cwd: string): string {
  return path.join(cwd, "node_modules/.inca/dev.pid");
}

/** Signal 0 checks for a process without touching it; `EPERM` means someone else owns it. */
function isRunning(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

/** Tells this run of the process apart from an earlier one that had the same pid. */
const token = randomUUID();

/**
 * Builds the error for a lock that another `inca dev` holds.
 *
 * @param pid - the pid recorded in the lock
 * @returns the error to throw
 */
function alreadyRunning(pid: number): IncaError {
  return new IncaError(
    "ERR_INCA_DEV_RUNNING",
    `inca dev is already running for this app (pid ${String(pid)})`,
  );
}

/**
 * Claims `cwd` for this process. Two `inca dev`s on one app watch the same
 * files and interleave their output on one terminal, and an orphan left by
 * an earlier run stays invisible until this says so. A lock whose pid is
 * gone, or that an earlier run of this process left behind, is taken over.
 *
 * @param cwd - the app directory to claim
 * @returns a function releasing the lock, safe to call more than once.
 * @throws if another live `inca dev` holds `cwd`.
 */
export async function acquireDevLock(cwd: string): Promise<() => Promise<void>> {
  const file = lockPath(cwd);
  const content = `${String(process.pid)}:${token}`;
  const temp = `${file}.${randomUUID()}`;
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(temp, content);

  let holder = process.pid;
  try {
    for (let attempt = 0; attempt < 5; attempt++) {
      try {
        await link(temp, file);
        return releaser(file, content);
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      }
      const recorded = await readFile(file, "utf8").catch(() => "");
      const pid = Number.parseInt(recorded, 10);
      if (pid === process.pid) {
        if (recorded === content) throw alreadyRunning(pid);
      } else if (Number.isSafeInteger(pid) && pid > 0 && isRunning(pid)) {
        throw alreadyRunning(pid);
      }
      holder = Number.isSafeInteger(pid) && pid > 0 ? pid : holder;
      // Two starts that find the same stale lock can both end up holding it.
      await rm(file, { force: true });
    }
    throw alreadyRunning(holder);
  } finally {
    await rm(temp, { force: true });
  }
}

/**
 * Builds the function that releases a lock this call wrote.
 *
 * @param file - the lock file
 * @param content - what this call wrote into it
 * @returns a function removing `file` once, and only while it still holds `content`
 */
function releaser(file: string, content: string): () => Promise<void> {
  let released = false;
  return async () => {
    if (released) return;
    released = true;
    if ((await readFile(file, "utf8").catch(() => "")) === content) await rm(file, { force: true });
  };
}
