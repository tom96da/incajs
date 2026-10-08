// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { RolldownError } from "rolldown";
import type { Logger } from "vite";

/** How rolldown opens the aggregated report it prints for a failed build. */
const AGGREGATE_REPORT = "Build failed with ";

/**
 * How Vite opens its own eager pre-warm of an HMR update's static imports.
 * A real fetch for the same file always follows; `hmr.mts`'s `onError`
 * reports that one instead.
 */
const PRE_TRANSFORM_ERROR = "Pre-transform error";

/**
 * A Vite {@link Logger} writing to the given streams: `info` to `stdout`,
 * `warn`/`error` to `stderr`, all unfiltered by Vite's own `LogOptions` — a
 * custom logger bypasses those. `quiet` drops `info` only; `warn`/`error`
 * still write, since they report something broken, not just what changed.
 */
export function streamLogger(
  stdout: NodeJS.WritableStream,
  stderr: NodeJS.WritableStream,
  quiet = false,
): Logger {
  const loggedErrors = new WeakSet<Error | RolldownError>();
  const warned = new Set<string>();

  const logger: Logger = {
    hasWarned: false,
    info(msg) {
      if (!quiet) stdout.write(`${msg}\n`);
    },
    warn(msg) {
      logger.hasWarned = true;
      stderr.write(`${msg}\n`);
    },
    warnOnce(msg) {
      if (warned.has(msg)) return;
      warned.add(msg);
      logger.warn(msg);
    },
    error(msg, options) {
      logger.hasWarned = true;
      if (options?.error) loggedErrors.add(options.error);
      // The CLI reports a failed build itself, from the watcher's ERROR
      // event. Matched anywhere in the message: on a tty the bundler's
      // color escapes come first.
      if (msg.includes(AGGREGATE_REPORT)) return;
      if (msg.includes(PRE_TRANSFORM_ERROR)) return;
      stderr.write(`${msg.replace(/\n\s+at .*/g, "")}\n`);
    },
    clearScreen() {},
    hasErrorLogged(error) {
      return loggedErrors.has(error);
    },
  };

  return logger;
}
