// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";

export interface ScratchApp {
  entry: string;
  outDir: string;
  vuePath: string;
}

/**
 * A scratch-app scaffold scoped to its own directory under `tests/tmp/`
 * (gitignored) rather than a real OS tmpdir: this mirrors how Vite
 * resolves a real app's `@vue/runtime-core` import, by walking up to this
 * package's own `node_modules`. Each caller gets its own `name` so
 * concurrently running test files never race on the same directory.
 */
export function scratchApp(name: string): {
  setUp: () => Promise<string | undefined>;
  tearDown: () => Promise<void>;
  makeApp: (vueSource: string) => Promise<ScratchApp>;
} {
  const scratchRoot = path.join(import.meta.dirname, "tmp", name);

  return {
    setUp: () => mkdir(scratchRoot, { recursive: true }),
    tearDown: () => rm(scratchRoot, { recursive: true, force: true }),
    async makeApp(vueSource: string): Promise<ScratchApp> {
      const appDir = await mkdtemp(path.join(scratchRoot, "app-"));
      const vuePath = path.join(appDir, "App.vue");
      const entry = path.join(appDir, "entry.mts");
      await writeFile(vuePath, vueSource);
      await writeFile(entry, `import App from "./App.vue";\nexport default App;\n`);
      return { entry, outDir: path.join(appDir, "dist"), vuePath };
    },
  };
}
