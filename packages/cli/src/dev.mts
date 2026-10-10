// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readdir, rm } from "node:fs/promises";
import path from "node:path";
import { styleText } from "node:util";

import {
  assertOutDir,
  resolveAppConfig,
  resolveBuildConfig,
  resolveRuntimeConfig,
  runtimeConfigOf,
} from "./config/loader.mts";
import { defaultBundler } from "./defaultBundler.mts";
import { HostClient, assertHostBin, resolveHostBin } from "./dev-client/index.mts";
import { acquireDevLock } from "./devLock.mts";
import { resolveEntry } from "./entry.mts";
import { IncaError } from "./error.mts";
import { STAMPED, log, printFault, toFault } from "./log.mts";
import { writeMacosApp } from "./macosApp.mts";
import { retryOnEdit } from "./retryOnEdit.mts";
import type { Bundler, BuildOutput, HmrChannel } from "./adapter/types.mts";
import type { ResolvedAppConfig } from "./config/loader.mts";

export interface DevOptions {
  /** The app's root directory. Defaults to `process.cwd()`. */
  cwd?: string;
  /**
   * The app's entry point. Defaults to resolving it automatically: a
   * committed `src/main.mts`, `src/main.ts` or `src/main.js`, or `src/App.vue` wrapped in a synthesized one.
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
  hostBin: string,
  stdout: NodeJS.WritableStream,
): Promise<string | undefined> {
  if (process.platform !== "darwin" || !metadata) return undefined;

  try {
    const app = await writeMacosApp({
      appPath: path.join(cwd, "node_modules/.inca", `${metadata.productName}.app`),
      metadata,
      hostBin,
      link: true,
    });
    return app.executablePath;
  } catch (error) {
    log(stdout, `running unbundled — ${toFault(error).message}`, STAMPED);
    return undefined;
  }
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
 * The environment to start the host with: the current one, plus
 * `FORCE_COLOR=1` when `stderr` is a terminal, so the host colors output
 * its piped stderr would otherwise not. `NO_COLOR` and `FORCE_COLOR`, when
 * set non-empty, are left alone.
 */
function hostEnv(stderr: NodeJS.WritableStream): NodeJS.ProcessEnv | undefined {
  const { NO_COLOR, FORCE_COLOR } = process.env;
  if (!(stderr as { isTTY?: boolean }).isTTY || NO_COLOR || FORCE_COLOR) return undefined;
  return { ...process.env, FORCE_COLOR: "1" };
}

/**
 * Builds the app, starts the host once the first build lands (at once under
 * `experimentalHmr`), and reloads it on every rebuild until `options.signal`
 * aborts or the host exits on its own.
 *
 * A config or entry that can't be used before the first build is printed as
 * `build failed`. `dev` then retries after the next edit to the config or the
 * entry.
 *
 * @throws if another `inca dev` is already running for this app.
 * @throws if the host binary can't be resolved or isn't a file.
 * @throws if `experimentalHmr` is set and the bundler has no HMR support.
 * @throws if the operating system refuses a file watcher.
 * @throws if the host exits before it is ready, or exits non-zero or by a
 * signal other than SIGINT/SIGTERM afterwards.
 */
export async function dev(options: DevOptions): Promise<void> {
  const cwd = options.cwd ?? process.cwd();
  const releaseLock = await acquireDevLock(cwd);
  try {
    const bin = options.hostBin ?? resolveHostBin();
    assertHostBin(bin);
    const bundler: Bundler = options.bundler ?? defaultBundler;
    const stdout = options.stdout ?? process.stdout;
    const stderr = options.stderr ?? process.stderr;

    const hmr = options.experimentalHmr ? bundler.hmr?.bind(bundler) : undefined;
    if (options.experimentalHmr && !hmr) {
      throw new IncaError(
        "ERR_INCA_HMR_UNSUPPORTED",
        "--experimental-hmr was set, but the configured bundler has no HMR support",
      );
    }

    const resolved = await retryOnEdit({
      cwd,
      signal: options.signal,
      stdout,
      stderr,
      async attempt() {
        const config = await resolveBuildConfig(cwd);
        const outDir = path.resolve(cwd, config.outDir);
        const entry = options.entry ?? config.entry ?? (await resolveEntry(cwd));
        assertOutDir(cwd, outDir, entry);
        const metadata = await resolveMetadata(cwd, stdout);
        const runtimeConfig = metadata
          ? runtimeConfigOf(metadata)
          : await resolveRuntimeConfig(cwd);
        return { outDir, entry, metadata, runtimeConfig };
      },
    });
    if (!resolved) return;
    const { outDir, entry, metadata, runtimeConfig } = resolved;
    const hostBin = (await bundleExecutable(cwd, metadata, bin, stdout)) ?? bin;

    let client: HostClient | undefined;
    let ready = false;
    let pendingReload = false;
    let exitedEarly = false;
    let hostExit: { code: number | null; signal: NodeJS.Signals | null } | undefined;
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
        env: hostEnv(stderr),
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
        onExit: (code, signal) => {
          exitedEarly = !ready;
          hostExit = { code, signal };
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

    if (hmr) {
      const channel: HmrChannel = await hmr({
        entry,
        cwd,
        runtimeConfig,
        stdout,
        stderr,
        notify: (payload) => client?.notify("vite", payload),
        // A full reload needs a fresh Engine/Host, which only inca-host's
        // own `reload` RPC method gives it — a `"vite"` notification can't.
        reload: () => {
          if (ready) void reloadHost();
          else pendingReload = true;
        },
        onError: (error) => printFault(stderr, "build failed", error, STAMPED),
        onUpdate: ({ file, took }) => {
          // Before `ready`, a start that recovers after an edit is reported as `ready`.
          if (!ready) return;
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
    // Ctrl-C also signals the host's process group, so SIGINT/SIGTERM exits are a quit.
    if (
      hostExit &&
      hostExit.code !== 0 &&
      hostExit.signal !== "SIGINT" &&
      hostExit.signal !== "SIGTERM"
    ) {
      const { code, signal } = hostExit;
      throw new IncaError(
        "ERR_INCA_HOST_CRASHED",
        `the host exited with ${signal ? `signal ${signal}` : `code ${code}`}`,
      );
    }
  } finally {
    await releaseLock();
  }
}
