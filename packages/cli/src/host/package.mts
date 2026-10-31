// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";

import { IncaError } from "../error.mts";

/** The brotli-compressed host binary, relative to the root of a host package. */
export const PACKED_HOST_FILE = "bin/inca-host.br";

/** The versions a host package may have. The version becomes part of a directory name. */
export const HOST_VERSION = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;

/** What the `package.json` of a host package says about its binary. */
export interface HostPackage {
  /** The package's name, such as `@incajs/host-linux-x64`. */
  name: string;
  /** The package's directory. */
  dir: string;
  /** The package's version, matching {@link HOST_VERSION}. */
  version: string;
  /** The SHA-256 of the unpacked binary, as 64 lowercase hex digits. */
  sha256: string;
}

const HOST_OS = ["darwin", "linux"] as const;
const HOST_ARCH = ["arm64", "x64"] as const;

/** The target of a host package, such as `linux-x64`. */
export type TargetKey = `${(typeof HOST_OS)[number]}-${(typeof HOST_ARCH)[number]}`;

/** The target of a process and the npm package that carries the host binary for it. */
export interface HostTarget {
  /** The target, such as `linux-x64`. */
  key: TargetKey;
  /** The package's name, such as `@incajs/host-linux-x64`. */
  name: `@incajs/host-${TargetKey}`;
}

/**
 * The target of a process and its host package.
 * @param platform - the operating system, as `process.platform` spells it
 * @param arch - the CPU architecture, as `process.arch` spells it
 * @returns `undefined` when no host package is published for the pair
 */
export function hostTarget(
  platform: string = process.platform,
  arch: string = process.arch,
): HostTarget | undefined {
  const os = HOST_OS.find((name) => name === platform);
  const cpu = HOST_ARCH.find((name) => name === arch);
  if (!os || !cpu) return undefined;
  const key = `${os}-${cpu}` as const;
  return { key, name: `@incajs/host-${key}` };
}

/**
 * Finds an installed host package, resolved from this file.
 * @param name - the package's name
 * @returns the package's directory, or `undefined` when it isn't installed
 */
export function locateHostPackageDir(name: string): string | undefined {
  try {
    return path.dirname(createRequire(import.meta.url).resolve(`${name}/package.json`));
  } catch {
    return undefined;
  }
}

/**
 * The error for a host package whose binary can't be used.
 * @param name - the package's name
 * @param reason - what is wrong with the package
 * @returns the error to throw
 */
export function hostPackageCorrupt(name: string, reason: string): IncaError {
  return new IncaError(
    "ERR_INCA_HOST_PACKAGE_CORRUPT",
    `the host package ${name} is damaged: ${reason}. reinstall it`,
  );
}

/**
 * Reads what the `package.json` of a host package says about its binary.
 * @param name - the package's name
 * @param dir - the package's directory
 * @returns `undefined` when `package.json` has no `incajs.host` field (a workspace checkout)
 * @throws `ERR_INCA_HOST_PACKAGE_CORRUPT` when `package.json` can't be read,
 * `incajs.host.sha256` is not 64 lowercase hex digits or `version` is not a semantic version
 */
export async function readHostPackage(name: string, dir: string): Promise<HostPackage | undefined> {
  let packageJson: {
    version?: unknown;
    incajs?: { host?: { sha256?: unknown } | null } | null;
  } | null;
  try {
    packageJson = JSON.parse(await readFile(path.join(dir, "package.json"), "utf8"));
  } catch (cause) {
    throw hostPackageCorrupt(name, `package.json can't be read (${(cause as Error).message})`);
  }
  const host = packageJson?.incajs?.host;
  if (host === undefined) return undefined;
  const sha256 = host?.sha256;
  if (typeof sha256 !== "string" || !/^[0-9a-f]{64}$/.test(sha256)) {
    throw hostPackageCorrupt(name, "verification failed");
  }
  const version = packageJson?.version;
  if (typeof version !== "string" || !HOST_VERSION.test(version)) {
    throw hostPackageCorrupt(name, "invalid version");
  }
  return { name, dir, version, sha256 };
}
