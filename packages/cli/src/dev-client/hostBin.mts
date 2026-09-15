// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createRequire } from "node:module";
import path from "node:path";

/** Overrides host binary resolution — the only way tests point this at a stand-in. */
const HOST_BIN_ENV_VAR = "GPJS_UI_HOST_BIN";

export interface ResolveHostBinOptions {
  /**
   * The workspace build to fall back to when no per-platform package is
   * installed. `dev`/`build` want the fast-to-rebuild debug binary;
   * packaging an app wants `release`. Defaults to `"debug"`.
   */
  profile?: "debug" | "release";
}

/** The per-platform npm package name for this process, or `undefined` off it. */
function platformPackageName(): string | undefined {
  const os =
    process.platform === "darwin" || process.platform === "linux" ? process.platform : undefined;
  const arch = process.arch === "arm64" || process.arch === "x64" ? process.arch : undefined;
  return os && arch ? `@incajs/host-${os}-${arch}` : undefined;
}

/** Resolves the prebuilt binary a per-platform package carries, if one is installed. */
function resolveFromPlatformPackage(): string | undefined {
  const packageName = platformPackageName();
  if (!packageName) return undefined;

  try {
    return createRequire(import.meta.url).resolve(`${packageName}/bin/gpjs-ui-host`);
  } catch {
    return undefined;
  }
}

/**
 * Where `gpjs-ui-host` itself lives. `GPJS_UI_HOST_BIN` wins outright,
 * then the per-platform npm package for this OS/arch (`@incajs/host-<os>-<arch>`,
 * an `optionalDependency` of this package), then this workspace's own Cargo
 * build output — the only place a binary exists before a package installs
 * one.
 */
export function resolveHostBin(options: ResolveHostBinOptions = {}): string {
  const fromEnv = process.env[HOST_BIN_ENV_VAR];
  if (fromEnv) return fromEnv;

  const fromPackage = resolveFromPlatformPackage();
  if (fromPackage) return fromPackage;

  const suffix = process.platform === "win32" ? ".exe" : "";
  const profile = options.profile ?? "debug";
  return path.resolve(import.meta.dirname, `../../../../target/${profile}/gpjs-ui-host${suffix}`);
}
