// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { stat } from "node:fs/promises";
import path from "node:path";

import { HOST_BIN_ENV_VAR } from "./bin.mts";
import { inspectHostPackage } from "./inspect.mts";

/** The notice and license files a host package carries, in the order they are copied. */
const LICENSE_FILE_NAMES = ["NOTICE", "LICENSE-MIT", "LICENSE-APACHE"] as const;

/**
 * Finds the notice and license files of the installed host package. They describe the published
 * binary, so a host the user supplies has none.
 * @param hostBin - the host binary the user passed, if any
 * @returns the absolute paths of those of `NOTICE`, `LICENSE-MIT` and `LICENSE-APACHE` that
 * exist, or `[]` when `hostBin` or `INCA_HOST_BIN` is set or no host package with a binary is
 * installed
 */
export async function hostLicenseFiles(hostBin?: string): Promise<string[]> {
  if (hostBin !== undefined || process.env[HOST_BIN_ENV_VAR]) return [];
  const host = await inspectHostPackage();
  if (host.state !== "installed") return [];
  const found = await Promise.all(
    LICENSE_FILE_NAMES.map(async (name) => {
      const file = path.join(host.pkg.dir, name);
      return (await stat(file).catch(() => undefined))?.isFile() ? file : undefined;
    }),
  );
  return found.filter((file) => file !== undefined);
}
