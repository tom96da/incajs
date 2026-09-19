// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

/**
 * A scratch-app scaffold scoped to its own directory under `tests/tmp/`
 * (gitignored) rather than a real OS tmpdir — mirrors
 * `tests/adapter/vite/scratchApp.mts`, but for config-loading tests: an app
 * here needs to resolve `@incajs/cli/config` from a real file on disk, which
 * an OS tmpdir can't do. Each caller gets its own `name` so concurrently
 * running test files never race on the same directory.
 */
export function scratchConfigApp(name: string): {
  setUp: () => Promise<string | undefined>;
  tearDown: () => Promise<void>;
  makeApp: (pkg: object) => Promise<string>;
} {
  const scratchRoot = path.join(import.meta.dirname, "tmp", name);

  return {
    setUp: () => mkdir(scratchRoot, { recursive: true }),
    tearDown: () => rm(scratchRoot, { recursive: true, force: true }),
    async makeApp(pkg: object): Promise<string> {
      const appDir = await mkdtemp(path.join(scratchRoot, "app-"));
      // Node needs "type": "module" to load an inca.config.ts without a
      // reparse-as-ESM warning — every real app already has this.
      await writeFile(
        path.join(appDir, "package.json"),
        JSON.stringify({ type: "module", ...pkg }),
      );
      return appDir;
    },
  };
}
