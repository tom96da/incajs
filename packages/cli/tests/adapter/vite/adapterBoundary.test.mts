// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { globSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

const adapterDir = path.join(import.meta.dirname, "../../../src/adapter/vite");
const CONTRACT = [
  path.resolve(adapterDir, "../../adapterCore.mts"),
  path.resolve(adapterDir, "../types.mts"),
];
const SOURCE = /\.(?:mts|ts|mjs|js)$/;
const IS_TEST = /\.test\.[^.]+$/;
/** Every relative specifier: `from "x"`, `import "x"`, `import("x")` and `require("x")`. */
const SPECIFIER = /(?:\bfrom\s*|\bimport\s*\(?\s*|\brequire\s*\(\s*)(["'])(\.[^"']*)\1/g;

/** What `source`, a module at `file`, imports that the adapter's boundary forbids. */
function violations(file: string, source: string): string[] {
  const found: string[] = [];
  for (const match of source.matchAll(SPECIFIER)) {
    const target = path.resolve(path.dirname(file), match[2]!);
    if (target.startsWith(adapterDir + path.sep)) continue;
    const start = Math.max(
      source.lastIndexOf("import", match.index),
      source.lastIndexOf("export", match.index),
    );
    const typeOnly = /^(?:import|export)\s+type\b/.test(source.slice(start, match.index));
    if (!(typeOnly && CONTRACT.includes(target))) found.push(match[2]!);
  }
  return found;
}

describe("the Vite adapter's boundary", () => {
  const modules = globSync("**/*", { cwd: adapterDir }).filter(
    (file) => SOURCE.test(file) && !IS_TEST.test(file),
  );

  it("scans every source module in the directory", () => {
    const listed = readdirSync(adapterDir, { recursive: true, withFileTypes: true }).filter(
      (entry) => entry.isFile() && SOURCE.test(entry.name) && !IS_TEST.test(entry.name),
    );

    expect(modules).toHaveLength(listed.length);
  });

  it("imports no CLI code at runtime", () => {
    const offenders = modules.flatMap((file) =>
      violations(
        path.join(adapterDir, file),
        readFileSync(path.join(adapterDir, file), "utf8"),
      ).map((specifier) => `${file} imports ${specifier}`),
    );

    expect(offenders).toEqual([]);
  });
});
