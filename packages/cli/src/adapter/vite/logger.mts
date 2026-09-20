// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { RolldownError } from "rolldown";
import type { Logger } from "vite";

/** How rolldown opens the aggregated report it prints for a failed build. */
const AGGREGATE_REPORT = "Build failed with ";

/**
 * A Vite {@link Logger} writing to the given streams: `info` to `stdout`,
 * `warn` and `error` to `stderr`. Messages arrive fully formatted and are
 * written as they are; Vite's own `LogOptions` are ignored.
 */
export function streamLogger(stdout: NodeJS.WritableStream, stderr: NodeJS.WritableStream): Logger {
  const loggedErrors = new WeakSet<Error | RolldownError>();
  const warned = new Set<string>();

  const logger: Logger = {
    hasWarned: false,
    info(msg) {
      stdout.write(`${msg}\n`);
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
      // colour escapes come first.
      if (msg.includes(AGGREGATE_REPORT)) return;
      stderr.write(`${msg.replace(/\n\s+at .*/g, "")}\n`);
    },
    clearScreen() {},
    hasErrorLogged(error) {
      return loggedErrors.has(error);
    },
  };

  return logger;
}
