// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { build } from "./build.mts";
import { dev } from "./dev.mts";
import { log, printFault, toFault } from "./log.mts";
import { packageApp } from "./package.mts";

const USAGE = "Usage: inca <dev|build|package>";

/** Formats a path relative to the working directory, the way the bundler reports its own. */
function relative(target: string): string {
  return path.relative(process.cwd(), target);
}

/** Parses argv and runs the named subcommand: `dev`, `build`, or `package`. */
export async function run(argv: readonly string[] = process.argv): Promise<void> {
  const command = argv[2];

  if (command === "build") {
    try {
      const output = await build();
      log(process.stdout, `built ${relative(output.entryFile)}`);
    } catch (error) {
      printFault(process.stderr, "build failed", toFault(error));
      process.exitCode = 1;
    }
    return;
  }

  if (command === "package") {
    try {
      const { appPath } = await packageApp();
      log(process.stdout, `packaged ${relative(appPath)}`);
    } catch (error) {
      printFault(process.stderr, "package failed", toFault(error));
      process.exitCode = 1;
    }
    return;
  }

  if (command !== "dev") {
    process.stderr.write(`${USAGE}\n`);
    process.exitCode = 1;
    return;
  }

  const controller = new AbortController();
  const onSignal = (): void => {
    if (controller.signal.aborted) return;
    log(process.stderr, "shutting down", { timestamp: true });
    controller.abort();
  };
  // A wrapper script runner (e.g. `pnpm run`) commonly delivers SIGTERM to
  // its child directly on Ctrl-C, rather than relying on the terminal to
  // signal the whole process group — SIGINT alone left the host orphaned.
  process.on("SIGINT", onSignal);
  process.on("SIGTERM", onSignal);

  try {
    await dev({ signal: controller.signal });
  } catch (error) {
    printFault(process.stderr, "dev failed", toFault(error), { timestamp: true });
    process.exitCode = 1;
  }
}
