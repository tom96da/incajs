// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { spawnSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { withAppRoot } from "./config/appRoot.mts";
import { resolveRuntimeConfig } from "./config/loader.mts";
import { assertHostBin, resolveHostBin } from "./dev-client/index.mts";
import { IncaError } from "./error.mts";
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
  const hostBin = options.hostBin ?? resolveHostBin();
  assertHostBin(hostBin);
  const config = await resolveRuntimeConfig(cwd, { dev: true });

  const dir = path.join(cwd, "node_modules/.inca/config");
  const entry = path.join(dir, "bundle.js");
  const file = path.join(dir, "inca.json");
  await mkdir(dir, { recursive: true });
  await writeFile(entry, "");
  await writeFile(file, JSON.stringify(withAppRoot(config, cwd, dir), undefined, 2));

  const result = spawnSync(hostBin, ["--print-config", entry], { encoding: "utf8" });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    const fault = settingFault(result.stderr, file, config);
    if (fault) throw fault;
  }
  (options.stdout ?? process.stdout).write(result.stdout);
  (options.stderr ?? process.stderr).write(result.stderr);
  return result.status ?? 1;
}
