// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";

import { loadConfig } from "c12";

import { defaultConfig, defaultIdentifier, defaultProductName } from "./defaults.mts";
import type { IncaConfig } from "./types.mts";

/** The parts of an app's `package.json` this loader reads directly, rather than through c12. */
interface AppPackageJson {
  name?: string;
  version?: string;
}

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
  /** package.json's `"inca"` key is deprecated; callers warn when this is true. */
  usedPackageJsonKey: boolean;
}

/**
 * Loads just `entry`/`outDir` from `inca.config.ts`, with derived defaults.
 * Never touches `package.json`.
 *
 * @param cwd - the app's root directory
 * @returns `entry`/`outDir`, resolved to absolute paths
 */
export async function resolveBuildConfig(cwd: string): Promise<ResolvedBuildConfig> {
  const { config } = await loadConfig<IncaConfig>({
    cwd,
    name: "inca",
    packageJson: "inca",
    rcFile: false,
    globalRc: false,
    giget: false,
    extend: false,
  });

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
    throw new Error(`no package.json found at ${pkgPath}`);
  }
  const pkg = JSON.parse(raw) as AppPackageJson;

  const { config, layers } = await loadConfig<IncaConfig>({
    cwd,
    name: "inca",
    packageJson: "inca",
    rcFile: false,
    globalRc: false,
    giget: false,
    extend: false,
  });

  const pkgJsonLayer = layers?.find((layer) => layer.configFile === "package.json");
  const usedPackageJsonKey = Object.keys(pkgJsonLayer?.config ?? {}).length > 0;

  const productName = config.productName ?? defaultProductName(pkg.name);
  if (!productName) {
    throw new Error(`${pkgPath} needs a "name", or "productName" in inca.config.ts`);
  }

  const version = config.version ?? pkg.version ?? defaultConfig.version;
  const identifier = config.identifier ?? defaultIdentifier(productName);

  let icon: string | undefined;
  if (config.icon) {
    icon = path.resolve(cwd, config.icon);
    if (!existsSync(icon)) {
      throw new Error(`"icon" points to ${icon}, which doesn't exist`);
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
    entry,
    outDir,
    usedPackageJsonKey,
  };
}
