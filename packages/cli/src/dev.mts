// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readdir, rm } from "node:fs/promises";
import path from "node:path";
import { styleText } from "node:util";

import { buildErrorMessage } from "vite";
import type { RollupError } from "rolldown";

import {
  assertOutDir,
  resolveAppConfig,
  resolveBuildConfig,
  resolveRuntimeConfig,
  runtimeConfigOf,
} from "./config/loader.mts";
import { defaultBundler } from "./defaultBundler.mts";
import { HostClient, resolveHostBin } from "./dev-client/index.mts";
import { acquireDevLock } from "./dev-lock.mts";
import { resolveEntry } from "./entry.mts";
import { IncaError } from "./error.mts";
import { log, printFault, toFault } from "./log.mts";
import { writeMacosApp } from "./macos-app.mts";
import type { Bundler, BuildOutput, HmrChannel, UpdateError } from "./adapter/types.mts";
import type { ResolvedAppConfig } from "./config/loader.mts";
import type { Fault } from "./log.mts";

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
  /**
   * Delivers module-granular updates instead of a full reload on every
   * change, over `bundler.hmr` — see `--experimental-hmr`.
   */
  experimentalHmr?: boolean;
  /**
   * Overrides host binary resolution. On macOS this is the binary the
   * app bundle wraps; elsewhere it is launched directly.
   */
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
 * How long `reload` waits for the host before giving up. A wedged app would
 * otherwise hang this call forever, which in turn blocks teardown — an
 * aborted `dev()` waits for the in-flight rebuild's own reload before it can
 * stop the host at all.
 */
const RELOAD_TIMEOUT_MS = 10_000;

/** The failures that leave the app unnamed instead of stopping `inca dev`. */
const UNNAMED: readonly string[] = [
  "ERR_INCA_PACKAGE_JSON_NOT_FOUND",
  "ERR_INCA_PRODUCT_NAME_MISSING",
];

/**
 * The app's full config, or `undefined` when it names itself nowhere,
 * which is reported on `stdout`.
 *
 * @throws if the config can't be read for any other reason
 */
async function resolveMetadata(
  cwd: string,
  stdout: NodeJS.WritableStream,
): Promise<ResolvedAppConfig | undefined> {
  try {
    return await resolveAppConfig(cwd);
  } catch (error) {
    if (!(error instanceof IncaError) || !UNNAMED.includes(error.code)) throw error;
    log(stdout, `running unnamed — ${error.message}`, STAMPED);
    return undefined;
  }
}

/**
 * The executable to launch on macOS so the running app carries the name and
 * icon `metadata` declares: one inside a `.app` bundle under
 * `node_modules/.inca`, holding the host binary and an `Info.plist`. macOS
 * names the Dock tile and the Finder entry after the bundle a process
 * launched from.
 *
 * `undefined` on every other platform, and whenever the bundle can't be
 * assembled — reported on `stdout`, and the host is launched unbundled.
 */
async function bundleExecutable(
  cwd: string,
  metadata: ResolvedAppConfig | undefined,
  hostBin: string | undefined,
  stdout: NodeJS.WritableStream,
): Promise<string | undefined> {
  if (process.platform !== "darwin" || !metadata) return undefined;

  try {
    const app = await writeMacosApp({
      appPath: path.join(cwd, "node_modules/.inca", `${metadata.productName}.app`),
      metadata,
      hostBin: hostBin ?? resolveHostBin(),
      link: true,
    });
    return app.executablePath;
  } catch (error) {
    log(stdout, `running unbundled — ${toFault(error).message}`, STAMPED);
    return undefined;
  }
}

/**
 * Reduces a failed HMR update to a {@link Fault}. `plugin`/`id`/`frame`,
 * when present, come from Vite's own `buildErrorMessage`, already colored.
 * They follow the stack, so this detail sits right above `printFault`'s
 * tagged summary line instead of scrolling off above the stack.
 */
function faultOf(error: UpdateError): Fault {
  const detail = buildErrorMessage(error as unknown as RollupError, [], false);
  const stack = [error.stack, detail].filter((line) => line).join("\n");
  return { message: error.message, stack: stack || null, code: null };
}

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
  try {
    const outDir = path.resolve(cwd, config.outDir);
    const bundler: Bundler = options.bundler ?? defaultBundler;
    const stdout = options.stdout ?? process.stdout;
    const stderr = options.stderr ?? process.stderr;

    const entry = options.entry ?? config.entry ?? (await resolveEntry(cwd));
    assertOutDir(cwd, outDir, entry);
    const metadata = await resolveMetadata(cwd, stdout);
    const runtimeConfig = metadata ? runtimeConfigOf(metadata) : await resolveRuntimeConfig(cwd);
    const hostBin =
      (await bundleExecutable(cwd, metadata, options.hostBin, stdout)) ?? options.hostBin;

    let client: HostClient | undefined;
    let ready = false;
    let pendingReload = false;
    let exitedEarly = false;
    let queue = Promise.resolve();

    let stop = (): void => {};
    const stopped = new Promise<void>((resolve) => {
      stop = resolve;
    });

    async function reloadHost(): Promise<boolean> {
      try {
        await client?.call("reload", undefined, RELOAD_TIMEOUT_MS);
        return true;
      } catch (error) {
        printFault(stderr, "reload failed", toFault(error), STAMPED);
        return false;
      }
    }

    /** Starts the host against `entryFile` and keeps it in `client` once it's up. */
    async function startHost(
      entryFile: string,
      integrations?: Record<string, (params: unknown) => void>,
    ): Promise<void> {
      const next = new HostClient({
        entryFile,
        hostBin,
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
          exitedEarly = !ready;
          // The window is gone, so there is nothing left to rebuild for.
          log(stdout, "host exited — stopping", STAMPED);
          stop();
        },
        integrations,
      });
      try {
        await next.start();
      } catch (error) {
        printFault(stderr, "failed to start inca-host", toFault(error), STAMPED);
        return;
      }
      client = next;
    }

    async function onBuild(output: BuildOutput): Promise<void> {
      await pruneStaleFiles(output.outDir, output.files);

      if (!client) {
        await startHost(output.entryFile);
        return;
      }

      if (!ready) {
        // The host hasn't finished its first load yet — its entry path is
        // always the same, so it picks up this build's content on its own
        // once it gets there; a reload now would only race it.
        pendingReload = true;
        return;
      }

      if (!(await reloadHost())) return;
      if (output.changed) {
        const file = styleText("dim", path.relative(cwd, output.changed.file), { stream: stdout });
        const took = Date.now() - output.changed.at;
        log(stdout, `${styleText("green", "reload")} ${file} (${took}ms)`, STAMPED);
      }
    }

    let closable: { close(): Promise<void> };

    if (options.experimentalHmr) {
      if (!bundler.hmr) {
        throw new IncaError(
          "ERR_INCA_HMR_UNSUPPORTED",
          "--experimental-hmr was set, but the configured bundler has no HMR support",
        );
      }

      const channel: HmrChannel = await bundler.hmr({
        entry,
        cwd,
        runtimeConfig,
        stdout,
        stderr,
        notify: (payload) => client?.notify("vite", payload),
        // A full reload needs a fresh Engine/Host, which only inca-host's
        // own `reload` RPC method gives it — a `"vite"` notification can't.
        reload: () => void reloadHost(),
        onError: (error) => printFault(stderr, "build failed", faultOf(error), STAMPED),
        onUpdate: ({ file, took }) => {
          const rel = styleText("dim", path.relative(cwd, file), { stream: stdout });
          log(stdout, `${styleText("green", "update")} ${rel} (${took}ms)`, STAMPED);
        },
      });
      await startHost(channel.entryFile, { vite: (params) => channel.dispatch(params) });
      closable = channel;
      // No retry path exists for HMR mode the way a failed non-HMR start is
      // naturally retried on the next rebuild — stop rather than hang.
      if (!client) stop();
    } else {
      closable = await bundler.watch({
        entry,
        outDir,
        runtimeConfig,
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
    }

    if (options.signal.aborted) stop();
    else options.signal.addEventListener("abort", () => stop(), { once: true });
    await stopped;

    await queue;
    await client?.stop();
    await closable.close();
    if (exitedEarly) {
      throw new IncaError("ERR_INCA_HOST_EXITED_EARLY", "the host exited before it was ready");
    }
  } finally {
    await releaseLock();
  }
}
