// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import path from "node:path";

/**
 * Locates the host binary the e2e tests start.
 * @returns `target/debug/inca-host`, else `target/release/inca-host`
 * @throws if neither binary exists. The tests never build one.
 */
export function resolveTestHostBin(): string {
  const repoRoot = path.resolve(import.meta.dirname, "../..");
  const debug = path.join(repoRoot, "target/debug/inca-host");
  const release = path.join(repoRoot, "target/release/inca-host");
  if (existsSync(debug)) return debug;
  if (existsSync(release)) return release;
  throw new Error(
    `no inca-host binary found at ${debug} or ${release} — run \`cargo build -p inca-host\` first`,
  );
}
