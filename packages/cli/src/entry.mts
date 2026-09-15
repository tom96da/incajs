// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

/**
 * Resolves an app's entry point. `src/main.mts`, committed by the app,
 * wins outright and is used verbatim — full control over bootstrapping.
 * Otherwise `src/App.vue` is wrapped in a synthesized entry equivalent to
 * `createIncaApp(App).mount()`, so a plain `.vue` file is a whole app
 * with no configuration at all.
 */
export async function resolveEntry(cwd: string): Promise<string> {
  const mainPath = path.join(cwd, "src/main.mts");
  if (existsSync(mainPath)) return mainPath;

  const appPath = path.join(cwd, "src/App.vue");
  if (existsSync(appPath)) return synthesizeEntry(appPath, cwd);

  throw new Error(`no app entry found — expected ${mainPath} or ${appPath}`);
}

async function synthesizeEntry(appPath: string, cwd: string): Promise<string> {
  // Not under `outDir`: Vite refuses an entry whose directory is `outDir`
  // itself or a descendant of it, since a build's own root normally holds
  // its source rather than sitting inside what it writes.
  const dir = path.join(cwd, "node_modules/.inca");
  await mkdir(dir, { recursive: true });

  const entryPath = path.join(dir, "entry.mts");
  const importSpecifier = JSON.stringify(appPath.split(path.sep).join("/"));
  await writeFile(
    entryPath,
    `import { createIncaApp } from "incajs/vue";\n` +
      `import App from ${importSpecifier};\n` +
      `createIncaApp(App).mount();\n`,
  );
  return entryPath;
}
