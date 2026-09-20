// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readdir, rm } from "node:fs/promises";
import path from "node:path";

import { resolveBuildConfig } from "./config/loader.mts";
import { defaultBundler } from "./defaultBundler.mts";
import { HostClient } from "./dev-client/index.mts";
import { acquireDevLock } from "./dev-lock.mts";
import { resolveEntry } from "./entry.mts";
import { log, printFault, toFault } from "./log.mts";
import type { Bundler, BuildOutput } from "./adapter/types.mts";

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

/** `inca dev` stamps its lines the way Vite's dev server does. */
const STAMPED = { timestamp: true } as const;

/**
 * Deletes every file directly under `outDir` that `keep` doesn't name —
 * `outDir` isn't emptied between rebuilds (a build in progress may be
 * evaluating there), so a chunk or asset an edit stops producing would
 * otherwise linger indefinitely.
 */
async function pruneStaleFiles(outDir: string, keep: readonly string[]): Promise<void> {
  const wanted = new Set(keep);
  let entries;
  try {
    entries = await readdir(outDir, { recursive: true, withFileTypes: true });
  } catch {
    return;
  }
  await Promise.all(
    entries
      .filter((entry) => entry.isFile())
      .map((entry) => path.relative(outDir, path.join(entry.parentPath, entry.name)))
      .filter((relPath) => !wanted.has(relPath))
      .map((relPath) => rm(path.join(outDir, relPath), { force: true })),
  );
}

/**
 * Builds the app, starts `inca-host` once the first build lands, and
 * reloads it on every rebuild — until `options.signal` aborts or the host
 * exits on its own.
 *
 * @throws if another `inca dev` is already running for this app.
 */
export async function dev(options: DevOptions): Promise<void> {
  const cwd = options.cwd ?? process.cwd();
  const config = await resolveBuildConfig(cwd);
  const releaseLock = await acquireDevLock(cwd);
  const outDir = config.outDir;
  const bundler: Bundler = options.bundler ?? defaultBundler;
  const stdout = options.stdout ?? process.stdout;
  const stderr = options.stderr ?? process.stderr;

  const entry = options.entry ?? config.entry ?? (await resolveEntry(cwd));

  let client: HostClient | undefined;
  let ready = false;
  let pendingReload = false;
  let queue = Promise.resolve();

  let stop = (): void => {};
  const stopped = new Promise<void>((resolve) => {
    stop = resolve;
  });

  async function reloadHost(): Promise<void> {
    try {
      await client?.call("reload");
    } catch (error) {
      printFault(stderr, "reload failed", toFault(error), STAMPED);
    }
  }

  async function onBuild(output: BuildOutput): Promise<void> {
    await pruneStaleFiles(output.outDir, output.files);

    if (!client) {
      const next = new HostClient({
        entryFile: output.entryFile,
        hostBin: options.hostBin,
        onStderr: (line) => stderr.write(line),
        onReady: () => {
          ready = true;
          log(stdout, "ready", STAMPED);
          if (pendingReload) {
            pendingReload = false;
            void reloadHost();
          }
        },
        onAppError: (error) => printFault(stderr, "app error", error, STAMPED),
        onExit: () => {
          // The window is gone, so there is nothing left to rebuild for.
          log(stdout, "host exited — stopping", STAMPED);
          stop();
        },
      });
      try {
        await next.start();
      } catch (error) {
        printFault(stderr, "failed to start inca-host", toFault(error), STAMPED);
        return;
      }
      client = next;
      return;
    }

    if (!ready) {
      // The host hasn't finished its first load yet — its entry path is
      // always the same, so it picks up this build's content on its own
      // once it gets there; a reload now would only race it.
      pendingReload = true;
      return;
    }

    await reloadHost();
  }

  const watcher = await bundler.watch({
    entry,
    outDir,
    mode: "development",
    stdout,
    stderr,
    onBuild: (output) => {
      // Chained rather than fired independently: two rebuilds landing
      // before the host finishes starting would otherwise both see no
      // client yet and each start their own.
      queue = queue.then(() => onBuild(output));
    },
    onError: (error) => {
      printFault(stderr, "build failed", error, STAMPED);
    },
  });

  if (options.signal.aborted) stop();
  else options.signal.addEventListener("abort", () => stop(), { once: true });
  await stopped;

  await queue;
  await client?.stop();
  await watcher.close();
  await releaseLock();
}
