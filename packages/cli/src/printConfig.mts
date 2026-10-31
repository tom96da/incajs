// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import type { SpawnSyncReturns } from "node:child_process";

import { withAppRoot } from "./config/appRoot.mts";
import { resolveRuntimeConfig } from "./config/loader.mts";
import { IncaError } from "./error.mts";
import { assertHostBin, resolveHostBin } from "./host/index.mts";
import { warn } from "./log.mts";
import type { RuntimeConfig } from "./config/loader.mts";
import type { DevConfig, WindowConfig } from "./config/types.mts";

export interface PrintConfigOptions {
  /** The app's root directory. Defaults to `process.cwd()`. */
  cwd?: string;
  /** Overrides host binary resolution. */
  hostBin?: string;
  /** Receives the settings. Defaults to `process.stdout`. */
  stdout?: NodeJS.WritableStream;
  /** Receives the host's own report when it names no setting. Defaults to `process.stderr`. */
  stderr?: NodeJS.WritableStream;
}

/** How a message names the type of a setting's value. */
type Kind<T> = T extends number
  ? "a number"
  : T extends string
    ? "a string"
    : T extends boolean
      ? "true or false"
      : "an object";

/** The kind of each setting, keyed as `inca.config.ts` spells it. */
type Kinds = {
  [K in keyof RuntimeConfig as K extends "name" ? "productName" : Exclude<K, "root">]-?: Kind<
    NonNullable<RuntimeConfig[K]>
  >;
} & {
  [K in keyof WindowConfig as `window.${K}`]-?: Kind<NonNullable<WindowConfig[K]>>;
} & {
  [K in keyof DevConfig as `dev.${K}`]-?: Kind<NonNullable<DevConfig[K]>>;
};

/** The kind each setting must have. The type checker holds each word to the setting's type. */
const KIND_OF: Kinds = {
  productName: "a string",
  identifier: "a string",
  window: "an object",
  dev: "an object",
  "window.width": "a number",
  "window.height": "a number",
  "window.title": "a string",
  "window.resizable": "true or false",
  "window.minWidth": "a number",
  "window.minHeight": "a number",
  "dev.stackPaths": "a string",
};

/** The kind of `value`, in the words of {@link KIND_OF}. */
function kindOf(value: unknown): string {
  if (typeof value === "number") return "a number";
  if (typeof value === "string") return "a string";
  if (typeof value === "boolean") return "true or false";
  return "an object";
}

/**
 * The failure to report for the host's answer on `file`, when it names a setting. The host
 * writes `failed to read <file>: <path>: <reason>`, `<path>` a JSON array of the keys that lead
 * to the value. Only the path is read. `inca.json`'s `name` comes from `productName`; every
 * other key is spelled as in `inca.config.ts`.
 *
 * @param stderr - what the host wrote to stderr
 * @param file - the `inca.json` the host read
 * @param config - what the CLI wrote to `file`
 * @returns `undefined` for a report on no known setting, or one that is not in that form
 */
function settingFault(stderr: string, file: string, config: object): IncaError | undefined {
  const prefix = `failed to read ${file}: `;
  const line = stderr.split("\n")[0] ?? "";
  if (!line.startsWith(prefix)) return undefined;
  const rest = line.slice(prefix.length);
  const end = rest.indexOf("]: ");

  let keys: unknown;
  try {
    keys = JSON.parse(rest.slice(0, end + 1));
  } catch {
    return undefined;
  }
  if (!Array.isArray(keys)) return undefined;
  const [first, ...nested] = keys;
  const setting = [first === "name" ? "productName" : first, ...nested].join(".");
  const kind: string | undefined = KIND_OF[setting as keyof Kinds];
  if (!kind) return undefined;

  const value: unknown = keys.reduce<unknown>(
    (parent, key) => (parent as Record<string, unknown> | undefined)?.[key],
    config,
  );
  const must = kindOf(value) === kind ? "the host can't use it" : `it must be ${kind}`;
  return new IncaError(
    "ERR_INCA_CONFIG_INVALID",
    `${setting} in inca.config.ts is ${JSON.stringify(value)}. ${must}`,
  );
}

/**
 * Starts the host with `--print-config` on `entry`.
 *
 * @param hostBin - the host binary
 * @param entry - the entry the host reads its config beside
 * @param file - the `inca.json` beside `entry`
 * @param config - what `file` holds
 * @returns the host's answer
 * @throws if the host can't be started, or reports a setting it can't use
 */
function runPrintConfig(
  hostBin: string,
  entry: string,
  file: string,
  config: object,
): SpawnSyncReturns<string> {
  const result = spawnSync(hostBin, ["--print-config", entry], { encoding: "utf8" });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    const fault = settingFault(result.stderr, file, config);
    if (fault) throw fault;
  }
  return result;
}

/**
 * Asks the host whether it can use the config beside `entry`. The host reads the `inca.json` the
 * bundler wrote there. A file that is missing or not JSON reads as an empty config, and the
 * host's own report reaches the user.
 *
 * @param hostBin - the host binary
 * @param entry - the entry the host is about to load
 * @throws if the host can't be started or a signal ends it, or it reports a setting it can't use,
 * or a config file it can't read
 */
export function checkConfig(hostBin: string, entry: string): void {
  const file = path.join(path.dirname(entry), "inca.json");
  let config: object = {};
  try {
    config = JSON.parse(readFileSync(file, "utf8"));
  } catch {}
  const result = runPrintConfig(hostBin, entry, file, config);
  if (result.status === null) {
    throw new IncaError(
      "ERR_INCA_HOST_EXITED_EARLY",
      `the host exited with signal ${String(result.signal)} before it read the config`,
    );
  }
  if (result.status !== 0) {
    const line = result.stderr.split("\n")[0] ?? "";
    throw new IncaError(
      "ERR_INCA_CONFIG_INVALID",
      `the host can't read ${file}. it reported "${line}"`,
    );
  }
}

/**
 * Runs {@link checkConfig} on a build's `entry` with the host binary that resolves. Without one,
 * or when it can't be unpacked, the check is skipped with a warning, because a build needs no host.
 *
 * @param entry - the entry the build wrote
 * @param stderr - receives the warning
 * @throws if the check fails, or the host binary can't be resolved for another reason
 */
export async function checkBuiltConfig(
  entry: string,
  stderr: NodeJS.WritableStream,
): Promise<void> {
  let hostBin: string;
  try {
    hostBin = await resolveHostBin(process.cwd(), stderr);
    await assertHostBin(hostBin);
  } catch (error) {
    if (
      error instanceof IncaError &&
      (error.code === "ERR_INCA_HOST_BIN_UNRESOLVED" ||
        error.code === "ERR_INCA_HOST_BIN_NOT_FOUND")
    ) {
      warn(stderr, "no host binary found, so the config was not checked");
      return;
    }
    if (error instanceof IncaError && error.code === "ERR_INCA_HOST_UNPACK_FAILED") {
      warn(stderr, "failed to unpack the host binary, so the config was not checked");
      return;
    }
    throw error;
  }
  checkConfig(hostBin, entry);
}

/**
 * Prints the settings the host applies to the app as JSON. The host reads the app's config from
 * `node_modules/.inca/config/inca.json`, written as a build writes it beside the bundle.
 *
 * @returns the host's exit code: 0 after printing, non-zero when the host refused the config
 * with a report that names no setting, written to `stderr`
 * @throws if the config can't be loaded, a setting holds a value the host can't use, or the
 * host binary can't be resolved, isn't a file or can't be started
 */
export async function printConfig(options: PrintConfigOptions = {}): Promise<number> {
  const cwd = options.cwd ?? process.cwd();
  const hostBin = options.hostBin ?? (await resolveHostBin(cwd, options.stderr));
  await assertHostBin(hostBin);
  const config = await resolveRuntimeConfig(cwd, { dev: true });

  const dir = path.join(cwd, "node_modules/.inca/config");
  const entry = path.join(dir, "bundle.js");
  const file = path.join(dir, "inca.json");
  await mkdir(dir, { recursive: true });
  await writeFile(entry, "");
  await writeFile(file, JSON.stringify(withAppRoot(config, cwd, dir), undefined, 2));

  const result = runPrintConfig(hostBin, entry, file, config);
  (options.stdout ?? process.stdout).write(result.stdout);
  (options.stderr ?? process.stderr).write(result.stderr);
  return result.status ?? 1;
}
