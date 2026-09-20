// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createRequire } from "node:module";
import { IncaError } from "../error.mts";

/** Overrides host binary resolution — for a stand-in in tests, or a platform with no published binary yet. */
const HOST_BIN_ENV_VAR = "INCA_HOST_BIN";

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
    return createRequire(import.meta.url).resolve(`${packageName}/bin/inca-host`);
  } catch {
    return undefined;
  }
}

/**
 * Where `inca-host` itself lives: `INCA_HOST_BIN` if set, otherwise the
 * per-platform npm package for this OS/arch (`@incajs/host-<os>-<arch>`, an
 * `optionalDependency` of this package).
 * @throws if neither resolves — no override is set, and either this
 * platform has no published binary, or the optional dependency carrying
 * it failed to install.
 */
export function resolveHostBin(): string {
  const fromEnv = process.env[HOST_BIN_ENV_VAR];
  if (fromEnv) return fromEnv;

  const fromPackage = resolveFromPlatformPackage();
  if (fromPackage) return fromPackage;

  const packageName = platformPackageName();
  const platformNote = packageName
    ? `no ${packageName} package is installed for it`
    : `this platform (${process.platform}/${process.arch}) has no published package`;
  throw new IncaError(
    "ERR_INCA_HOST_BIN_UNRESOLVED",
    `no inca-host binary found — ${platformNote}, and ${HOST_BIN_ENV_VAR} isn't set to one`,
  );
}
