// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { stat } from "node:fs/promises";
import { homedir } from "node:os";

import { IncaError } from "../error.mts";
import { cacheRoots, unpackHost } from "./cache.mts";
import { inspectHostPackage } from "./inspect.mts";

/**
 * Overrides host binary resolution, for a stand-in in tests, or a platform with no published
 * binary yet.
 */
const HOST_BIN_ENV_VAR = "INCA_HOST_BIN";

/** The user's home directory, or `undefined` when the system reports none. */
function homeDirectory(): string | undefined {
  try {
    return homedir();
  } catch {
    return undefined;
  }
}

/**
 * Where the host binary lives: `INCA_HOST_BIN` if set, otherwise the binary of the per-platform
 * npm package for this OS/arch (`@incajs/host-<os>-<arch>`, an `optionalDependency` of this
 * package), unpacked on first use into the first root that takes it: `INCA_CACHE_DIR`, the user
 * cache, then `node_modules/.inca/host` of the app.
 * @param cwd - the app's root directory
 * @param stderr - receives the line that names the directory a new unpack wrote into
 * @returns the path of the host binary to start
 * @throws `ERR_INCA_HOST_BIN_UNRESOLVED` if no override is set and no package carries a binary
 * for this platform, `ERR_INCA_HOST_PACKAGE_CORRUPT` if the package's binary is unusable and
 * `ERR_INCA_HOST_UNPACK_FAILED` if no root can take it.
 */
export async function resolveHostBin(
  cwd: string = process.cwd(),
  stderr: NodeJS.WritableStream = process.stderr,
): Promise<string> {
  const fromEnv = process.env[HOST_BIN_ENV_VAR];
  if (fromEnv) return fromEnv;

  const host = await inspectHostPackage();
  if (host.state === "installed") {
    const roots = cacheRoots(process.env, process.platform, homeDirectory(), cwd);
    return unpackHost(host.pkg, host.target.key, roots, stderr);
  }
  if (host.state === "corrupt") throw host.error;

  const cause =
    host.state === "unsupported"
      ? `no host package is published for ${process.platform}/${process.arch}`
      : host.state === "noBinary"
        ? `${host.target.name} is installed but carries no host binary`
        : `${host.target.name} isn't installed`;
  throw new IncaError(
    "ERR_INCA_HOST_BIN_UNRESOLVED",
    `no host binary found because ${cause}. ` +
      `set ${HOST_BIN_ENV_VAR} to the path of a host binary to use your own`,
  );
}

/**
 * Checks that a host binary is a file.
 * @param bin - the path to check
 * @throws if nothing is a file at `bin`
 */
export async function assertHostBin(bin: string): Promise<void> {
  if (!(await stat(bin).catch(() => undefined))?.isFile()) {
    throw new IncaError(
      "ERR_INCA_HOST_BIN_NOT_FOUND",
      `no host binary at ${bin}. check that it was built`,
    );
  }
}
