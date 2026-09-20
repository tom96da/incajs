// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

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

/**
 * Claims `cwd` for this process. Two `inca dev`s on one app watch the same
 * files and interleave their output on one terminal, and an orphan left by
 * an earlier run stays invisible until this says so. A lock whose pid is
 * gone is taken over.
 *
 * @returns a function releasing the lock.
 * @throws if another live `inca dev` holds `cwd`.
 */
export async function acquireDevLock(cwd: string): Promise<() => Promise<void>> {
  const file = lockPath(cwd);
  const held = Number.parseInt(await readFile(file, "utf8").catch(() => ""), 10);
  if (Number.isInteger(held) && isRunning(held)) {
    throw new Error(`inca dev is already running for this app (pid ${String(held)})`);
  }

  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, String(process.pid));
  return () => rm(file, { force: true });
}
