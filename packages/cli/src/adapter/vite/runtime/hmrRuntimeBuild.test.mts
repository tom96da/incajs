// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { build, mergeConfig } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import viteConfig from "../../../../vite.config.mts";

const BARE_IMPORT_RE = /^\s*import\s+(?:type\s+)?[^"'()]*from\s*["']([^"']+)["']/gm;

function isRelative(specifier: string): boolean {
  return specifier.startsWith("./") || specifier.startsWith("../");
}

/**
 * Every import specifier a built file's `import ... from "..."` statements
 * use. Drops matches inside a template-literal string the built code
 * happens to contain (a specifier can't hold `${`, so those aren't real
 * import declarations).
 */
function importSpecifiers(source: string): string[] {
  return [...source.matchAll(BARE_IMPORT_RE)]
    .map((match) => match[1] ?? "")
    .filter((specifier) => !specifier.includes("${"));
}

/**
 * Reads `entryFile` and every file it imports, transitively following only
 * relative specifiers, and returns the set of files visited.
 */
async function readTransitiveImports(entryFile: string): Promise<Map<string, string>> {
  const visited = new Map<string, string>();
  const queue = [entryFile];
  while (queue.length > 0) {
    const file = queue.pop();
    if (!file || visited.has(file)) continue;
    const source = await readFile(file, "utf8");
    visited.set(file, source);
    for (const specifier of importSpecifiers(source)) {
      if (isRelative(specifier)) queue.push(path.resolve(path.dirname(file), specifier));
    }
  }
  return visited;
}

describe("hmr-runtime build output", () => {
  let outDir: string;

  beforeAll(async () => {
    outDir = await mkdtemp(path.join(tmpdir(), "inca-hmr-runtime-"));
    await build(
      mergeConfig(viteConfig, {
        build: { outDir, emptyOutDir: true },
        plugins: [],
        logLevel: "silent",
      }),
    );
  }, 30000);

  afterAll(async () => {
    await rm(outDir, { recursive: true, force: true });
  });

  it("bundles vite/module-runner in, leaving no bare or node: import behind", async () => {
    const files = await readTransitiveImports(path.join(outDir, "hmr-runtime.js"));

    expect(files.size).toBeGreaterThan(0);
    for (const [file, source] of files) {
      for (const specifier of importSpecifiers(source)) {
        expect(isRelative(specifier), `${file} imports "${specifier}"`).toBe(true);
      }
    }
  });
});
