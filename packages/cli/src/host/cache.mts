// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createHash, randomBytes } from "node:crypto";
import { constants } from "node:fs";
import {
  access,
  chmod,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { brotliDecompressSync } from "node:zlib";

import { IncaError } from "../error.mts";
import { log } from "../log.mts";
import { HOST_VERSION, PACKED_HOST_FILE, hostPackageCorrupt } from "./package.mts";
import type { HostPackage, TargetKey } from "./package.mts";

/** Sets the directory the CLI tries first when it unpacks the host binary. */
export const CACHE_DIR_ENV_VAR = "INCA_CACHE_DIR";

/** How many unpacked binaries of one target stay in a root. */
const KEPT_ENTRIES = 3;

/** The unpacked file's name, so a process list shows `inca-host`. */
const UNPACKED_NAME = "inca-host";

/**
 * A directory that can hold unpacked host binaries. {@link cacheRoots} lists them in the order they
 * are tried.
 */
export interface CacheRoot {
  /** The directory that holds the entries. */
  dir: string;
  /** A directory that must exist already. The CLI doesn't create it. */
  mustExist?: string;
}

/** The SHA-256 of `bytes` as lowercase hex. */
function sha256Of(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}

/** The user cache of a platform, or `undefined` when there is no home directory. */
function userCacheRoot(
  env: NodeJS.ProcessEnv,
  platform: string,
  home: string | undefined,
): CacheRoot | undefined {
  const xdg = env.XDG_CACHE_HOME;
  if (platform !== "darwin" && xdg && path.isAbsolute(xdg)) {
    return { dir: path.join(xdg, "incajs", "host") };
  }
  if (!home) return undefined;
  return {
    dir:
      platform === "darwin"
        ? path.join(home, "Library", "Caches", "incajs", "host")
        : path.join(home, ".cache", "incajs", "host"),
  };
}

/**
 * The roots to try, in order: `INCA_CACHE_DIR` when set, the user cache, then
 * `node_modules/.inca/host` of the app.
 * @param env - the process environment
 * @param platform - `process.platform`
 * @param home - the user's home directory, `undefined` or `""` when there is none
 * @param cwd - the app's root directory
 * @returns the roots without duplicates
 */
export function cacheRoots(
  env: NodeJS.ProcessEnv,
  platform: string,
  home: string | undefined,
  cwd: string,
): CacheRoot[] {
  const roots: CacheRoot[] = [];
  const override = env[CACHE_DIR_ENV_VAR];
  if (override) roots.push({ dir: path.resolve(override) });
  const userCache = userCacheRoot(env, platform, home);
  if (userCache) roots.push(userCache);
  roots.push({
    dir: path.join(cwd, "node_modules/.inca/host"),
    mustExist: path.join(cwd, "node_modules"),
  });
  return roots.filter(
    (root, index) => roots.findIndex((other) => other.dir === root.dir) === index,
  );
}

/** True when `file` holds the binary with this hash and the process may run it. */
async function isUsable(file: string, sha256: string): Promise<boolean> {
  try {
    if (sha256Of(await readFile(file)) !== sha256) return false;
    await access(file, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/** Decompresses the package's binary and checks it against `package.json`. */
async function unpack(pkg: HostPackage): Promise<Buffer> {
  let binary: Buffer;
  try {
    binary = brotliDecompressSync(await readFile(path.join(pkg.dir, PACKED_HOST_FILE)));
  } catch (cause) {
    throw hostPackageCorrupt(
      pkg.name,
      `${PACKED_HOST_FILE} can't be unpacked (${(cause as Error).message})`,
    );
  }
  if (sha256Of(binary) !== pkg.sha256) {
    throw hostPackageCorrupt(pkg.name, "verification failed");
  }
  return binary;
}

/** Writes `binary` to `file` through a temporary file in the same directory, then renames it. */
async function writeEntry(file: string, binary: Buffer): Promise<void> {
  const temp = `${file}.tmp-${process.pid}-${randomBytes(4).toString("hex")}`;
  try {
    await mkdir(path.dirname(file), { recursive: true, mode: 0o700 });
    await writeFile(temp, binary);
    await chmod(temp, 0o755);
    await access(temp, constants.X_OK);
    await rename(temp, file);
  } catch (error) {
    await rm(temp, { force: true });
    throw error;
  }
}

/**
 * Removes the entries of `target` in `root` beyond the newest {@link KEPT_ENTRIES}, keeping
 * `keep`.
 */
async function prune(root: string, target: TargetKey, keep: string): Promise<void> {
  const entryName = new RegExp(`^${target}@${HOST_VERSION.source.slice(1)}`);
  try {
    const dirs = (await readdir(root, { withFileTypes: true }))
      .filter((item) => item.isDirectory() && entryName.test(item.name) && item.name !== keep)
      .map((item) => path.join(root, item.name));
    const others = (
      await Promise.all(dirs.map(async (dir) => ({ dir, modified: (await stat(dir)).mtimeMs })))
    )
      .sort((a, b) => b.modified - a.modified)
      .slice(KEPT_ENTRIES - 1);
    for (const { dir } of others) await rm(dir, { recursive: true, force: true });
  } catch {
    // The launch has succeeded by now. A directory another user owns stays.
  }
}

/** True when `dir` is a directory this process can reach. */
async function isDirectory(dir: string): Promise<boolean> {
  try {
    return (await stat(dir)).isDirectory();
  } catch {
    return false;
  }
}

/**
 * Makes the host binary of a package available as a file this process can run.
 * @param pkg - the host package, from `readHostPackage`
 * @param target - the key of the target, from `hostTarget`
 * @param roots - the roots to try in order, from {@link cacheRoots}
 * @param stderr - receives the line that names the directory a new entry was unpacked into
 * @returns the path of the unpacked binary
 * @throws `ERR_INCA_HOST_PACKAGE_CORRUPT` when the package's binary can't be unpacked or doesn't
 * match its checksum, and `ERR_INCA_HOST_UNPACK_FAILED` when no root can take the file
 */
export async function unpackHost(
  pkg: HostPackage,
  target: TargetKey,
  roots: readonly CacheRoot[],
  stderr: NodeJS.WritableStream,
): Promise<string> {
  const entry = `${target}@${pkg.version}`;
  for (const root of roots) {
    const file = path.join(root.dir, entry, UNPACKED_NAME);
    if (await isUsable(file, pkg.sha256)) return file;
  }

  let binary: Buffer | undefined;
  for (const root of roots) {
    if (root.mustExist && !(await isDirectory(root.mustExist))) continue;
    binary ??= await unpack(pkg);
    const file = path.join(root.dir, entry, UNPACKED_NAME);
    try {
      await writeEntry(file, binary);
    } catch {
      continue;
    }
    log(stderr, `unpacked the host binary into ${path.dirname(file)}`);
    await prune(root.dir, target, entry);
    return file;
  }
  throw new IncaError(
    "ERR_INCA_HOST_UNPACK_FAILED",
    "failed to unpack the host binary because no usable directory was found to unpack into. " +
      `set ${CACHE_DIR_ENV_VAR} to a writable directory that allows running programs`,
  );
}
