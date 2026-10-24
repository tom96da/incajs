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
import { entryRoot, resolveEntry, selectEntry } from "./entry.mts";
import { IncaError } from "./error.mts";
import { STAMPED, log, printFault, toFault } from "./log.mts";
import { writeMacosApp } from "./macosApp.mts";
import { retryOnEdit } from "./retryOnEdit.mts";
import { watchForEdit } from "./watchForEdit.mts";
import type { ResolvedAppConfig } from "./config/loader.mts";
import type { Bundler, BuildOutput, HmrChannel } from "./types.mts";
import type { EditWatch } from "./watchForEdit.mts";

export interface DevOptions {
  /** The app's root directory. Defaults to `process.cwd()`. */
  cwd?: string;
  /**
   * The app's entry point. Defaults to the first of `src/main.mts`, `src/main.ts` and
   * `src/main.js` that exists, otherwise `src/App.vue` wrapped in a synthesized entry. An entry
   * selected this way is selected again while the app runs, and the host reloads when the
   * selection changes.
   */
  entry?: string;
  /** Overrides the bundler. See {@link defaultBundler} for what's wired in by default. */
  bundler?: Bundler;
  /**
   * Delivers module-granular updates instead of a full reload on every
   * change, over `bundler.hmr`. See `--experimental-hmr`.
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
 * How long `reload` waits for the host before giving up. A wedged app never
 * answers, and an aborted `dev()` waits for the in-flight rebuild's reload
 * before it can stop the host.
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
    log(stdout, `running unnamed. ${error.message}`, STAMPED);
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
 * assembled. It is reported on `stdout`, and the host is launched unbundled.
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
    log(stdout, `running unbundled. ${toFault(error).message}`, STAMPED);
    return undefined;
  }
}

/**
 * Deletes every file directly under `outDir` that `keep` doesn't name.
 * `outDir` isn't emptied between rebuilds (a build in progress may be
 * evaluating there), so a chunk or asset an edit stops producing is deleted
 * here.
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

/** A bundler watcher or HMR channel serving one entry. */
interface Session {
  entry: string;
  /** Ignores the session's later callbacks and closes its watcher or channel. */
  close(): Promise<void>;
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
 * When the entry is selected automatically, `dev` selects it again after every change to a main
 * file or `src/App.vue` and reloads the development window if the selection changed. A missing
 * entry prints `build failed` and keeps the running host until one exists.
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
  let stop = (): void => {};
  let stopping = false;
  const stopped = new Promise<void>((resolve) => {
    stop = () => {
      stopping = true;
      resolve();
    };
  });
  let entryWatch: EditWatch | undefined;
  let monitoring: Promise<{ error: unknown } | undefined> | undefined;
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
        const automaticEntry = options.entry === undefined && config.entry === undefined;
        const entry = options.entry ?? config.entry ?? (await resolveEntry(cwd));
        assertOutDir(cwd, outDir, entry);
        const metadata = await resolveMetadata(cwd, stdout);
        const runtimeConfig = metadata
          ? runtimeConfigOf(metadata)
          : await resolveRuntimeConfig(cwd);
        return { outDir, entry, automaticEntry, metadata, runtimeConfig };
      },
    });
    if (!resolved) return;
    const { outDir, entry, automaticEntry, metadata, runtimeConfig } = resolved;
    const hostBin = (await bundleExecutable(cwd, metadata, bin, stdout)) ?? bin;

    let client: HostClient | undefined;
    let ready = false;
    let pendingReload = false;
    let exitedEarly = false;
    let hostExit: { code: number | null; signal: NodeJS.Signals | null } | undefined;
    let queue = Promise.resolve();
    let session: Session | undefined;
    // A session's callbacks run only while this still holds the value its start gave it.
    let generation = 0;
    // True from the report of a missing entry until an entry is selected again.
    let missing = false;
    // The open HMR channel's dispatch, which the host's `vite` integration calls.
    let dispatchVite: ((params: unknown) => void) | undefined;

    async function reloadHost(): Promise<boolean> {
      try {
        await client?.call("reload", undefined, RELOAD_TIMEOUT_MS);
        return true;
      } catch (error) {
        printFault(stderr, "reload failed", toFault(error), STAMPED);
        return false;
      }
    }

    function logReload(changed: { file: string; at: number } | undefined): void {
      if (!changed) return;
      const file = styleText("dim", path.relative(cwd, changed.file), { stream: stdout });
      const took = Date.now() - changed.at;
      log(stdout, `${styleText("green", "reload")} ${file} (${took}ms)`, STAMPED);
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
          log(stdout, "host exited. stopping", STAMPED);
          stop();
        },
        integrations,
      });
      try {
        await next.start();
      } catch (error) {
        printFault(stderr, "failed to start the host", toFault(error), STAMPED);
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
        // The host hasn't finished its first load yet. Its entry path is
        // always the same, so it picks up this build's content on its own
        // once it gets there; a reload now would only race it.
        pendingReload = true;
        return;
      }

      if (!(await reloadHost())) return;
      logReload(output.changed);
    }

    /**
     * Opens the watcher or HMR channel for `entry`. The host starts with its first bundle, or at
     * once under HMR. A host that is already running reloads with the new entry's bundle.
     */
    async function openSession(entry: string): Promise<Session> {
      assertOutDir(cwd, outDir, entry);
      const mine = ++generation;
      // `stopping` is set the moment shutdown starts, before the session closes.
      const current = (): boolean => mine === generation && !stopping;
      const live =
        <Args extends unknown[]>(callback: (...args: Args) => void) =>
        (...args: Args): void => {
          if (current()) callback(...args);
        };
      let closable: { close(): Promise<void> };

      if (hmr) {
        const channel: HmrChannel = await hmr({
          entry,
          root: entryRoot(cwd, entry),
          envDir: cwd,
          publicDir: path.join(cwd, "public"),
          cwd,
          runtimeConfig,
          stdout,
          stderr,
          notify: live((payload) => client?.notify("vite", payload)),
          // A full reload needs a fresh Engine/Host, which only inca-host's
          // own `reload` RPC method gives it. A `"vite"` notification can't.
          reload: live((changed) => {
            if (ready) {
              void reloadHost().then((ok) => {
                if (ok) logReload(changed);
              });
            } else {
              pendingReload = true;
            }
          }),
          onError: live((error) => printFault(stderr, "build failed", error, STAMPED)),
          onUpdate: live(({ file, took }) => {
            // Before `ready`, a start that recovers after an edit is reported as `ready`.
            if (!ready) return;
            const rel = styleText("dim", path.relative(cwd, file), { stream: stdout });
            log(stdout, `${styleText("green", "update")} ${rel} (${took}ms)`, STAMPED);
          }),
        });
        dispatchVite = (params) => channel.dispatch(params);
        closable = {
          close: () => {
            dispatchVite = undefined;
            return channel.close();
          },
        };
        if (!client) {
          await startHost(channel.entryFile, { vite: (params) => dispatchVite?.(params) });
          // No retry path exists for HMR mode the way a failed non-HMR start is
          // naturally retried on the next rebuild.
          if (!client) stop();
        } else if (ready) {
          await reloadHost();
        } else {
          pendingReload = true;
        }
      } else {
        closable = await bundler.watch({
          entry,
          root: entryRoot(cwd, entry),
          envDir: cwd,
          publicDir: path.join(cwd, "public"),
          outDir,
          runtimeConfig,
          mode: "development",
          stdout,
          stderr,
          onBuild: (output) => {
            // Chained rather than fired independently: two rebuilds landing
            // before the host finishes starting would otherwise both see no
            // client yet and each start their own. A session closed while
            // its build waits here must not reach the host.
            queue = queue.then(() => (current() ? onBuild(output) : undefined));
          },
          onError: live((error) => {
            printFault(stderr, "build failed", error, STAMPED);
          }),
        });
      }

      return {
        entry,
        async close() {
          generation++;
          await closable.close();
        },
      };
    }

    async function closeSession(): Promise<void> {
      const closing = session;
      session = undefined;
      await closing?.close();
    }

    /**
     * Moves to the entry that is selected now. Runs in `queue`, so it never overlaps a build
     * callback and never rejects.
     */
    async function reselect(): Promise<void> {
      if (stopping) return;
      const selected = selectEntry(cwd);
      if (selected !== undefined && selected === session?.entry) return;
      try {
        // Closing the session stops repeat build errors. The host stays.
        if (selected === undefined) await closeSession();
        const next = await resolveEntry(cwd);
        missing = false;
        await closeSession();
        if (stopping) return;
        session = await openSession(next);
      } catch (error) {
        if (!missing) printFault(stderr, "build failed", toFault(error), STAMPED);
        missing = selected === undefined;
      }
    }

    /** Selects the entry again after every change to a file that decides it. */
    async function monitorEntry(first: EditWatch): Promise<void> {
      let watch = first;
      // `stopped` comes first so that it wins over an edit that lands with it.
      while (await Promise.race([stopped.then(() => false), watch.edited.then(() => true)])) {
        watch.close();
        watch = entryWatch = watchForEdit(cwd);
        queue = queue.then(reselect);
        await queue;
      }
    }

    let initial: string | undefined = entry;
    if (automaticEntry) {
      // Armed before the recheck, so a change made after it is not lost.
      entryWatch = watchForEdit(cwd);
      try {
        initial = await resolveEntry(cwd);
      } catch (error) {
        initial = undefined;
        missing = true;
        printFault(stderr, "build failed", toFault(error), STAMPED);
      }
    }
    if (initial !== undefined) session = await openSession(initial);
    if (entryWatch) {
      monitoring = monitorEntry(entryWatch).then(
        () => undefined,
        (error: unknown) => {
          stop();
          return { error };
        },
      );
    }

    if (options.signal.aborted) stop();
    else options.signal.addEventListener("abort", () => stop(), { once: true });
    await stopped;

    const monitorFailure = await monitoring;
    await session?.close();
    await queue;
    await client?.stop();
    if (monitorFailure) throw monitorFailure.error;
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
    // Ends the entry monitor and waits for it, so it arms no watch after this.
    stop();
    await monitoring;
    entryWatch?.close();
    await releaseLock();
  }
}
