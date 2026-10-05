// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import vue from "@vitejs/plugin-vue";
import { DevEnvironment, buildErrorMessage, createServer } from "vite";
import type { RollupError } from "rolldown";
import type {
  CustomPayload,
  HotChannel,
  HotChannelClient,
  HotPayload,
  Plugin,
  ViteDevServer,
} from "vite";

import { CONFIG_FILE_NAME, serializeConfig } from "./emitConfig.mts";
import { streamLogger } from "./logger.mts";
import { rejectUnsupported } from "./unsupported.mts";
import type { AdapterCore } from "../../adapterCore.mts";
import type { BuildFailure, HmrChannel, HmrOptions } from "../types.mts";

/**
 * A failed update's detail — the same fields a Rollup/Vite plugin
 * error carries beyond `message`/`stack`, when it has them.
 */
interface UpdateError {
  message: string;
  stack?: string | null;
  plugin?: string | null;
  id?: string | null;
  frame?: string | null;
  loc?: { line: number; column: number } | null;
}

/**
 * Reduces a failed update to a {@link BuildFailure}, with the message and
 * `code` a failed rebuild reports. Plugin, file and frame detail from Vite's
 * `buildErrorMessage` follows the stack.
 */
export function faultOfUpdate(core: AdapterCore, error: UpdateError): BuildFailure {
  const detail = buildErrorMessage(error as unknown as RollupError, [], false);
  const stack = [error.stack, detail].filter((line) => line).join("\n");
  return { ...core.bundlerFault(error.message), stack: stack || null };
}

/** The directory a session's synthesized entry and config are written to. */
function hmrDirOf(cwd: string): string {
  return path.join(cwd, "node_modules/.inca/hmr");
}

/**
 * `dist/hmr-runtime.js`, resolved relative to this package's own root
 * rather than this module's own location — correct whether this file is
 * running from `src/` (tests, the workspace) or a built `dist/`.
 */
function resolveHmrRuntimePath(): string {
  let dir = path.dirname(fileURLToPath(import.meta.url));
  while (!existsSync(path.join(dir, "package.json"))) {
    const parent = path.dirname(dir);
    if (parent === dir) throw new Error("could not locate @incajs/cli's own package.json");
    dir = parent;
  }
  return path.join(dir, "dist/hmr-runtime.js");
}

/** A relative ES module specifier from `fromDir` to `target`, forward-slashed. */
function importSpecifierTo(fromDir: string, target: string): string {
  const rel = path.relative(fromDir, target).split(path.sep).join("/");
  return rel.startsWith(".") ? rel : `./${rel}`;
}

/**
 * Writes the entry `inca-host` evaluates for an HMR session: it starts the
 * bundled module runner fire-and-forget, so evaluating it never waits on
 * the runner's first `fetchModule` round-trip.
 */
async function writeHmrEntry(cwd: string, entry: string, runtimePath?: string): Promise<string> {
  const dir = hmrDirOf(cwd);
  await mkdir(dir, { recursive: true });

  const entryPath = path.join(dir, "entry.js");
  const runtimeImport = importSpecifierTo(dir, runtimePath ?? resolveHmrRuntimePath());
  const entryId = entry.split(path.sep).join("/");
  await writeFile(
    entryPath,
    `import { start } from ${JSON.stringify(runtimeImport)};\n` +
      `start(${JSON.stringify(entryId)}).catch((err) => console.error(err));\n`,
  );
  return entryPath;
}

/**
 * Pulls a failed update's detail out of the two shapes it reaches this
 * channel in: a direct `type: "error"` push, or a `fetchModule` RPC's own
 * error response (the shape `vite:invoke`'s response wraps it in).
 */
function updateErrorOf(payload: HotPayload): UpdateError | undefined {
  if (payload.type === "error") return payload.err;
  if (payload.type === "custom" && payload.event === "vite:invoke") {
    return (payload.data as { data?: { error?: UpdateError } })?.data?.error;
  }
  return undefined;
}

/**
 * Whether `payload` is a successful `fetchModule` `vite:invoke` response —
 * the module runner's own RPC for pulling an updated module's compiled
 * code, and the actual verdict on an HMR update. An `update` payload only
 * announces that a fetch is coming; its own failure is caught separately
 * by {@link updateErrorOf} on this same response.
 */
function isFetchModuleSuccess(payload: HotPayload): boolean {
  if (payload.type !== "custom" || payload.event !== "vite:invoke") return false;
  const invoke = payload.data as { name?: string; data?: { error?: unknown } };
  return invoke.name === "fetchModule" && !invoke.data?.error;
}

/**
 * A minimal server-side {@link HotChannel}. `send` relays to `notify`.
 * A `full-reload` payload calls `reload` instead: that needs a fresh
 * `Engine`/`Host`, which only `inca-host`'s `reload` RPC gives it.
 * A failed update also calls `onError`; a successful one calls `onUpdate`
 * with how long it took since `changed` last saw a file, when there was
 * one waiting — gated on the module runner's own `fetchModule` response,
 * so a broken edit never reports success ahead of its own failure. Either
 * way the payload is still relayed to the app afterward, since its own
 * pending call may be waiting on it. `dispatch` feeds a payload the app
 * sent back into `DevEnvironment`'s own `fetchModule`/HMR machinery.
 */
function createIncaHotChannel(
  notify: (payload: unknown) => void,
  reload: () => void,
  onError: (error: UpdateError) => void,
  onUpdate: (info: { file: string; took: number }) => void,
  changed: { current: { file: string; at: number } | undefined },
): { channel: HotChannel; dispatch: (payload: unknown) => void } {
  const listeners = new Map<string, Set<(data: unknown, client: HotChannelClient) => void>>();
  const client: HotChannelClient = { send: outbound };

  function outbound(payload: HotPayload): void {
    if (payload.type === "full-reload") {
      reload();
      return;
    }
    const error = updateErrorOf(payload);
    if (error) {
      onError(error);
      changed.current = undefined;
    } else if (isFetchModuleSuccess(payload) && changed.current) {
      onUpdate({ file: changed.current.file, took: Date.now() - changed.current.at });
      changed.current = undefined;
    }
    notify(payload);
  }

  const channel: HotChannel = {
    send: outbound,
    on(event: string, listener: (data: unknown, client: HotChannelClient) => void) {
      let forEvent = listeners.get(event);
      if (!forEvent) listeners.set(event, (forEvent = new Set()));
      forEvent.add(listener);
    },
    off(event, listener) {
      listeners.get(event)?.delete(listener as (data: unknown, client: HotChannelClient) => void);
    },
  };

  return {
    channel,
    dispatch(payload) {
      const custom = payload as Partial<CustomPayload>;
      if (custom?.type !== "custom" || typeof custom.event !== "string") return;
      for (const listener of listeners.get(custom.event) ?? []) listener(custom.data, client);
    },
  };
}

/**
 * Stubs out `/@vite/client` (the browser HMR client import analysis
 * injects wherever `import.meta.hot` appears) — the module runner's own
 * `hot` getter creates the real context instead.
 *
 * Vite's built-in `resolve.alias` entry for `/@vite/client` runs before any
 * plugin's own `resolveId`, rewriting it to an `/@fs/…/dist/client/client.mjs`
 * absolute id first — so this has to match the resolved path, not the raw
 * specifier, or the real client (which opens a WebSocket) loads instead.
 */
function shimViteClient(): Plugin {
  const shimId = "\0inca:vite-client-shim";
  return {
    name: "inca:shim-vite-client",
    enforce: "pre",
    resolveId(id) {
      return id === "/@vite/client" || id.endsWith("/dist/client/client.mjs") ? shimId : null;
    },
    load(id) {
      return id === shimId ? "export function createHotContext() { return undefined; }" : null;
    },
  };
}

/**
 * Forwards each changed file to `@vitejs/plugin-vue`'s own HMR support,
 * which only ever sends `file-changed` over the server-wide WebSocket
 * client — one this custom environment doesn't have. Also records when
 * this happened, for `createIncaHotChannel`'s own `onUpdate` timing.
 */
function fileChangedPlugin(changed: { current: { file: string; at: number } | undefined }): Plugin {
  return {
    name: "inca:file-changed",
    hotUpdate(options) {
      changed.current = { file: options.file, at: Date.now() };
      this.environment.hot.send({
        type: "custom",
        event: "file-changed",
        data: { file: options.file },
      } satisfies CustomPayload);
    },
  };
}

/**
 * Starts a Vite dev server driving the app over its dev channel instead of
 * a browser: a `"client"` environment with a custom {@link HotChannel}
 * standing in for the WebSocket server `server.ws: false` disables.
 */
export async function hmr(
  core: AdapterCore,
  {
    entry,
    cwd,
    runtimeConfig,
    runtimePath,
    stdout = process.stdout,
    stderr = process.stderr,
    quiet = false,
    notify,
    reload,
    onError,
    onUpdate = () => {},
  }: HmrOptions,
): Promise<HmrChannel> {
  const entryFile = await writeHmrEntry(cwd, entry, runtimePath);

  const configContent = serializeConfig(runtimeConfig);
  const configPath = path.join(hmrDirOf(cwd), CONFIG_FILE_NAME);
  if (configContent) await writeFile(configPath, configContent);
  else await rm(configPath, { force: true });

  const changed: { current: { file: string; at: number } | undefined } = { current: undefined };
  const { channel, dispatch } = createIncaHotChannel(
    notify,
    reload,
    (error) => onError(faultOfUpdate(core, error)),
    onUpdate,
    changed,
  );

  const server: ViteDevServer = await createServer({
    configFile: false,
    root: path.dirname(entry),
    mode: "development",
    clearScreen: false,
    logLevel: quiet ? "silent" : "info",
    customLogger: streamLogger(stdout, stderr, quiet),
    define: { "process.env.NODE_ENV": JSON.stringify("development") },
    server: { middlewareMode: true, ws: false },
    optimizeDeps: { noDiscovery: true },
    environments: {
      client: {
        consumer: "client",
        dev: {
          moduleRunnerTransform: true,
          createEnvironment: (name, config) =>
            new DevEnvironment(name, config, { hot: true, transport: channel }),
        },
      },
    },
    plugins: [
      vue({
        template: {
          // Hoisted static content is stringified into createStaticVNode, which needs insertStaticContent.
          compilerOptions: { runtimeModuleName: "@vue/runtime-core", hoistStatic: false },
        },
      }),
      rejectUnsupported(core),
      shimViteClient(),
      fileChangedPlugin(changed),
    ],
  });

  return {
    entryFile,
    dispatch,
    close: () => server.close(),
  };
}
