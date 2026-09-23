// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";

import { loadConfig } from "c12";

import { IncaError } from "../error.mts";
import { defaultConfig, defaultIdentifier, defaultProductName } from "./defaults.mts";
import type { IncaConfig, WindowConfig } from "./types.mts";

/** The parts of an app's `package.json` this loader reads itself. */
interface AppPackageJson {
  name?: string;
  version?: string;
}

/**
 * What a config load looks at: an `inca.config.*` beside the app, and
 * `package.json`'s deprecated `"inca"` key. Nothing else — no rc file, no
 * global config, no remote template.
 */
const CONFIG_LOOKUP = {
  name: "inca",
  packageJson: "inca",
  rcFile: false,
  globalRc: false,
  giget: false,
  extend: false,
} as const;

/** `entry`/`outDir` only — what `build()`/`dev()` need, with no `package.json` requirement. */
export interface ResolvedBuildConfig {
  /** Absolute path, if `inca.config.ts` set one. */
  entry?: string;
  /** Absolute path. */
  outDir: string;
}

/** An app's full config, as `inca package` needs it. */
export interface ResolvedAppConfig extends ResolvedBuildConfig {
  productName: string;
  identifier: string;
  /** True when `identifier` was generated rather than set explicitly. */
  identifierIsDefault: boolean;
  version: string;
  /** Absolute path, if the app declared one. */
  icon?: string;
  /** What the app declared about its window, if anything. */
  window?: WindowConfig;
  /** package.json's `"inca"` key is deprecated; callers warn when this is true. */
  usedPackageJsonKey: boolean;
}

/**
 * What a built app carries for its host to read: the name the platform
 * shows it under, the id it is known by, and its window. Each field is
 * absent unless the app's config sets it.
 */
export interface RuntimeConfig {
  name?: string;
  identifier?: string;
  window?: WindowConfig;
}

/** Characters no directory name may hold, on any platform this runs on. */
const RESERVED_IN_A_NAME = /[/\\:*?"<>|\0]/;

/** Whether `name` is usable as a directory name. */
function namesADirectory(name: string): boolean {
  return (
    name === name.trim() &&
    name !== "" &&
    name !== "." &&
    name !== ".." &&
    !RESERVED_IN_A_NAME.test(name)
  );
}

/**
 * @param productName - a resolved display name
 * @throws if it can't name the directory a packaged app becomes
 */
function assertUsableProductName(productName: unknown): asserts productName is string {
  if (typeof productName === "string" && namesADirectory(productName)) return;
  throw new IncaError(
    "ERR_INCA_PRODUCT_NAME_INVALID",
    `${JSON.stringify(productName)} can't name a directory — set a "productName" in ` +
      `inca.config.ts with no leading or trailing space, and none of \`/\\:*?"<>|\``,
  );
}

/** Loads `inca.config.ts`, and reports whether `package.json`'s `"inca"` key supplied anything. */
async function loadIncaConfig(
  cwd: string,
): Promise<{ config: IncaConfig; usedPackageJsonKey: boolean }> {
  const { config, layers } = await loadConfig<IncaConfig>({ cwd, ...CONFIG_LOOKUP });
  const pkgJsonLayer = layers?.find((layer) => layer.configFile === "package.json");
  return { config, usedPackageJsonKey: Object.keys(pkgJsonLayer?.config ?? {}).length > 0 };
}

/**
 * Loads just `entry`/`outDir` from `inca.config.ts`, with derived defaults.
 * Never touches `package.json`.
 *
 * @param cwd - the app's root directory
 * @returns `entry`/`outDir`, resolved to absolute paths
 */
export async function resolveBuildConfig(cwd: string): Promise<ResolvedBuildConfig> {
  const { config } = await loadIncaConfig(cwd);

  return {
    entry: config.entry ? path.resolve(cwd, config.entry) : undefined,
    outDir: path.resolve(cwd, config.outDir ?? defaultConfig.outDir),
  };
}

/**
 * Loads an app's full config for `inca package`: `inca.config.ts` over
 * `package.json`'s `"inca"` key over derived defaults.
 *
 * @param cwd - the app's root directory
 * @returns the resolved config
 * @throws if `package.json` is missing, has neither a `name` nor a
 * `productName`, or an `icon` doesn't resolve to a real file
 */
export async function resolveAppConfig(cwd: string): Promise<ResolvedAppConfig> {
  const pkgPath = path.join(cwd, "package.json");
  let raw: string;
  try {
    raw = await readFile(pkgPath, "utf8");
  } catch {
    throw new IncaError("ERR_INCA_PACKAGE_JSON_NOT_FOUND", `no package.json found at ${pkgPath}`);
  }
  const pkg = JSON.parse(raw) as AppPackageJson;

  const { config, usedPackageJsonKey } = await loadIncaConfig(cwd);

  const productName = config.productName ?? defaultProductName(pkg.name);
  if (!productName) {
    throw new IncaError(
      "ERR_INCA_PRODUCT_NAME_MISSING",
      `${pkgPath} needs a "name", or "productName" in inca.config.ts`,
    );
  }
  assertUsableProductName(productName);

  const version = config.version ?? pkg.version ?? defaultConfig.version;
  const identifier = config.identifier ?? defaultIdentifier(productName);

  let icon: string | undefined;
  if (config.icon) {
    icon = path.resolve(cwd, config.icon);
    if (!existsSync(icon)) {
      throw new IncaError(
        "ERR_INCA_ICON_NOT_FOUND",
        `"icon" points to ${icon}, which doesn't exist`,
      );
    }
  }

  const entry = config.entry ? path.resolve(cwd, config.entry) : undefined;
  const outDir = path.resolve(cwd, config.outDir ?? defaultConfig.outDir);

  return {
    productName,
    identifier,
    identifierIsDefault: config.identifier === undefined,
    version,
    icon,
    window: config.window,
    entry,
    outDir,
    usedPackageJsonKey,
  };
}

/**
 * Narrows a full config down to what a build carries for the host.
 *
 * @param config - a config {@link resolveAppConfig} returned
 * @returns its name, id and window
 */
export function runtimeConfigOf(config: ResolvedAppConfig): RuntimeConfig {
  return {
    name: config.productName,
    identifier: config.identifier,
    window: config.window,
  };
}

/**
 * Loads {@link RuntimeConfig} for an app that may name itself nowhere.
 * An app with no `package.json`, or none that gives it a name, still gets
 * whatever window settings it declared.
 *
 * @param cwd - the app's root directory
 * @returns the config, or `undefined` when the app declares none of it
 */
export async function resolveRuntimeConfig(cwd: string): Promise<RuntimeConfig | undefined> {
  const { config } = await loadIncaConfig(cwd);

  let pkg: AppPackageJson = {};
  try {
    pkg = JSON.parse(await readFile(path.join(cwd, "package.json"), "utf8")) as AppPackageJson;
  } catch {
    // An app without a readable package.json names itself only through
    // inca.config.ts, if at all.
  }

  const name = config.productName ?? defaultProductName(pkg.name);
  if (name) assertUsableProductName(name);
  const identifier = config.identifier ?? (name ? defaultIdentifier(name) : undefined);
  const runtime: RuntimeConfig = { name, identifier, window: config.window };

  return Object.values(runtime).some((value) => value !== undefined) ? runtime : undefined;
}
