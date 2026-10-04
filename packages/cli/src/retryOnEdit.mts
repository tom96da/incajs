// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";

import { IncaError } from "./error.mts";
import { STAMPED, log, printFault, toFault } from "./log.mts";
import { watchForEdit } from "./watchForEdit.mts";
import type { EditWatch } from "./watchForEdit.mts";

/** Options for {@link retryOnEdit}. */
export interface RetryOnEditOptions<T> {
  /** The app's root directory. */
  cwd: string;
  /** Aborting ends the wait. */
  signal: AbortSignal;
  /** Receives the progress lines. */
  stdout: NodeJS.WritableStream;
  /** Receives each failure. */
  stderr: NodeJS.WritableStream;
  /** How long edits must stay quiet before the next attempt. Defaults to 100. */
  debounceMs?: number;
  /**
   * One try. A thrown value is printed and the attempt repeats after the
   * next relevant edit. `watch` is live for the whole try.
   */
  attempt(watch: EditWatch): Promise<T>;
}

/**
 * Resolves `true` once `edited` does, or `false` once `signal` aborts.
 *
 * @param edited - the promise of an {@link EditWatch}
 * @param signal - the signal ending the wait
 */
function waitForEdit(edited: Promise<void>, signal: AbortSignal): Promise<boolean> {
  return new Promise((resolve) => {
    const onAbort = (): void => resolve(false);
    signal.addEventListener("abort", onAbort, { once: true });
    void edited.then(() => {
      signal.removeEventListener("abort", onAbort);
      resolve(true);
    });
  });
}

/**
 * Runs `attempt` until it succeeds. A failure is printed as `build failed`,
 * then the call waits for an edit to a file that may fix it and tries again.
 * An `IncaError` that names a `path` adds that file to the watch. When that
 * file already exists, the call retries once at once and prints nothing. A
 * second failure in a row is printed and waits for an edit.
 *
 * @param options - the attempt and where to report
 * @returns the value of the first attempt that succeeds, or `undefined` when `signal` aborts first
 * @throws if the operating system refuses a watcher. The failure that led to
 * a refused `add` is printed first.
 */
export async function retryOnEdit<T>(options: RetryOnEditOptions<T>): Promise<T | undefined> {
  const { cwd, signal, stdout, stderr, debounceMs } = options;
  // A silent retry is bound to one in a row. The wait resets it.
  let silentRetry = false;
  for (;;) {
    // Armed before the attempt, so a save made during it counts.
    const watch = watchForEdit(cwd, { debounceMs });
    try {
      return await options.attempt(watch);
    } catch (error) {
      const file = error instanceof IncaError ? error.path : undefined;
      if (file) {
        try {
          watch.add(file);
        } catch (addError) {
          // The watcher failure ends the call, so the failure behind it is printed first.
          printFault(stderr, "build failed", toFault(error), STAMPED);
          throw addError;
        }
      }
      if (!signal.aborted && !silentRetry && file && existsSync(file)) {
        silentRetry = true;
        continue;
      }
      silentRetry = false;

      printFault(stderr, "build failed", toFault(error), STAMPED);
      if (signal.aborted) return undefined;
      log(stdout, "waiting for a change to retry", STAMPED);
      if (!(await waitForEdit(watch.edited, signal))) return undefined;
      log(stdout, "retrying", STAMPED);
    } finally {
      watch.close();
    }
  }
}
