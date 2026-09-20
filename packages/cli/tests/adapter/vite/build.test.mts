// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { build } from "../../../src/adapter/vite/index.mts";
import { scratchApp } from "./scratchApp.mts";

const { setUp, tearDown, makeApp } = scratchApp("build");

beforeAll(setUp);
afterAll(tearDown);

describe("build", () => {
  it("writes Vite's own build log to the stream it was given", async () => {
    const { entry, outDir, streams, logs } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    await build({ entry, outDir, ...streams });

    expect(logs()).toContain("built in");
  }, 20000);

  it("compiles a .vue file into a minified entry, reporting it and every file it wrote", async () => {
    const { entry, outDir, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const output = await build({ entry, outDir, ...streams });

    expect(output.outDir).toBe(outDir);
    expect(output.entryFile).toBe(path.join(outDir, "bundle.js"));
    expect(output.files).toContain("bundle.js");

    const bundle = await readFile(output.entryFile, "utf8");
    expect(bundle).toContain("hello");
    // Vue's own readable helper names survive an unminified (dev) build —
    // their absence here is what distinguishes a production build from one.
    expect(bundle).not.toContain("createElementBlock");
  }, 20000);

  it("reports a dynamic import as its own chunk alongside the entry", async () => {
    const { entry, outDir, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );
    const dynamicEntry = path.join(path.dirname(entry), "dynamic-entry.mts");
    await writeFile(
      dynamicEntry,
      `import("./App.vue").then((m) => { globalThis.loaded = m.default; });\n`,
    );

    const output = await build({ entry: dynamicEntry, outDir, ...streams });

    expect(output.files.length).toBeGreaterThan(1);
    expect(output.files.some((file) => file.startsWith("chunks/"))).toBe(true);
  }, 20000);

  it("clears a stale file already in outDir", async () => {
    const { entry, outDir, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );
    await mkdir(outDir, { recursive: true });
    const stalePath = `${outDir}/stale.js`;
    await writeFile(stalePath, "// leftover from a previous build\n");

    await build({ entry, outDir, ...streams });

    await expect(readFile(stalePath, "utf8")).rejects.toThrow(/ENOENT/);
  }, 20000);

  it("rejects a <style> block, naming the file that used it", async () => {
    const { entry, outDir, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n` +
        `<style scoped>\ndiv { color: red; }\n</style>\n`,
    );

    await expect(build({ entry, outDir, ...streams })).rejects.toThrow(
      /App\.vue uses a <style> block/,
    );
  }, 20000);

  it("rejects with a readable error on a syntax error", async () => {
    const { entry, outDir, streams } = await makeApp(
      `<script setup>\nconst broken = ;\n</script>\n<template><div/></template>\n`,
    );

    await expect(build({ entry, outDir, ...streams })).rejects.toThrow(/./);
  }, 20000);
});
