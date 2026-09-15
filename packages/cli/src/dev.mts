// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { defaultBundler } from "./defaultBundler.mts";
import { HostClient } from "./dev-client/index.mts";
import { resolveEntry } from "./entry.mts";
import { printFault, toFault } from "./fault.mts";
import type { Bundler } from "./adapter/types.mts";

export interface DevOptions {
  /** The app's root directory. Defaults to `process.cwd()`. */
  cwd?: string;
  /**
   * The app's entry point. Defaults to resolving it automatically: a
   * committed `src/main.mts`, or `src/App.vue` wrapped in a synthesized one.
   */
  entry?: string;
  /** Overrides the bundler — see {@link defaultBundler} for what's wired in by default. */
  bundler?: Bundler;
  /** Overrides host binary resolution — passed straight through to `HostClient`. */
  hostBin?: string;
  /** Defaults to `process.stdout`. */
  stdout?: NodeJS.WritableStream;
  /** Defaults to `process.stderr`. */
  stderr?: NodeJS.WritableStream;
  /** Aborting tears the host and the bundler watcher down and resolves `dev()`. */
  signal: AbortSignal;
}

/**
 * Builds the app, starts `gpjs-ui-host` once the first bundle lands, and
 * reloads it on every rebuild — until `options.signal` aborts.
 */
export async function dev(options: DevOptions): Promise<void> {
  const cwd = options.cwd ?? process.cwd();
  const outDir = path.join(cwd, "dist");
  const bundler: Bundler = options.bundler ?? defaultBundler;
  const stdout = options.stdout ?? process.stdout;
  const stderr = options.stderr ?? process.stderr;

  const entry = options.entry ?? (await resolveEntry(cwd));

  let client: HostClient | undefined;
  let ready = false;
  let pendingReload = false;
  let queue = Promise.resolve();

  async function reloadHost(): Promise<void> {
    try {
      await client?.call("reload");
    } catch (error) {
      printFault(stderr, "reload failed", toFault(error));
    }
  }

  async function onBuild(bundlePath: string): Promise<void> {
    if (!client) {
      const next = new HostClient({
        bundlePath,
        hostBin: options.hostBin,
        onStderr: (line) => stderr.write(line),
        onReady: () => {
          ready = true;
          stdout.write("[gpjsui] ready\n");
          if (pendingReload) {
            pendingReload = false;
            void reloadHost();
          }
        },
        onAppError: (error) => printFault(stderr, "app error", error),
      });
      try {
        await next.start();
      } catch (error) {
        printFault(stderr, "failed to start gpjs-ui-host", toFault(error));
        return;
      }
      client = next;
      return;
    }

    if (!ready) {
      // The host hasn't finished its first load yet — bundlePath is
      // always the same file, so it picks up this build's content on its
      // own once it gets there; a reload now would only race it.
      pendingReload = true;
      return;
    }

    await reloadHost();
  }

  const watcher = await bundler.watch({
    entry,
    outDir,
    mode: "development",
    onBuild: (bundlePath) => {
      // Chained rather than fired independently: two rebuilds landing
      // before the host finishes starting would otherwise both see no
      // client yet and each start their own.
      queue = queue.then(() => onBuild(bundlePath));
    },
    onError: (error) => {
      printFault(stderr, "build failed", error);
    },
  });

  await new Promise<void>((resolve) => {
    if (options.signal.aborted) {
      resolve();
      return;
    }
    options.signal.addEventListener("abort", () => resolve(), { once: true });
  });

  await queue;
  await client?.stop();
  await watcher.close();
}
