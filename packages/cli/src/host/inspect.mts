// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { hostTarget, locateHostPackageDir, readHostPackage } from "./package.mts";
import type { IncaError } from "../error.mts";
import type { HostPackage, HostTarget } from "./package.mts";

/** What is installed for this platform. */
export type HostPackageState =
  /** No host package is published for this OS and CPU. */
  | { state: "unsupported" }
  | { state: "notInstalled"; target: HostTarget }
  /** The package is installed and its `package.json` has no `incajs.host`. */
  | { state: "noBinary"; target: HostTarget }
  /** The installed package is damaged. `error` says how. */
  | { state: "corrupt"; target: HostTarget; error: IncaError }
  | { state: "installed"; target: HostTarget; pkg: HostPackage };

/**
 * Looks up the host package of this platform.
 * @returns what is installed, with the package when it carries a binary
 */
export async function inspectHostPackage(): Promise<HostPackageState> {
  const target = hostTarget();
  if (!target) return { state: "unsupported" };
  const dir = locateHostPackageDir(target.name);
  if (!dir) return { state: "notInstalled", target };
  try {
    const pkg = await readHostPackage(target.name, dir);
    return pkg ? { state: "installed", target, pkg } : { state: "noBinary", target };
  } catch (error) {
    return { state: "corrupt", target, error: error as IncaError };
  }
}
