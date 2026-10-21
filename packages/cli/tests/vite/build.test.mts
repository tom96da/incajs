// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import dedent from "ts-dedent";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import * as adapterCore from "../../src/adapterCore.mts";
import { createViteBundler } from "../../src/vite/index.mts";
import { scratchApp, TEST_RUNTIME_MODULE } from "./scratchApp.mts";
import type { Bundler } from "../../src/types.mts";

const vite = createViteBundler(adapterCore, TEST_RUNTIME_MODULE);
const build: Bundler["build"] = (options) => vite.build(options);

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
    // Vue's own readable helper names survive an unminified (dev) build.
    // Their absence here is what distinguishes a production build from one.
    expect(bundle).not.toContain("createElementBlock");
  }, 20000);

  it("leaves static siblings unstringified", async () => {
    const { entry, outDir, streams } = await makeApp(
      `<template><div>${"<div>a</div>".repeat(25)}</div></template>\n`,
    );

    const output = await build({ entry, outDir, ...streams });

    // Stringified static content survives as literal markup in the bundle.
    expect(await readFile(output.entryFile, "utf8")).not.toContain("<div>a</div>");
  }, 20000);

  it("imports the modifier helpers from the runtime core module", async () => {
    const { entry, outDir, streams } = await makeApp(
      dedent`
        <script setup>
        import { ref } from "@vue/runtime-core";
        const n = ref(0);
        </script>
        <template><div @click.stop="n++" @keydown.enter="n++">{{ n }}</div></template>
      `,
    );

    const output = await build({ entry, outDir, ...streams });

    const bundle = await readFile(output.entryFile, "utf8");
    expect(bundle).toContain("STUB_WITH_MODIFIERS");
    expect(bundle).toContain("STUB_WITH_KEYS");
    expect(bundle).toContain('["stop"]');
    expect(bundle).toContain('["enter"]');
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

  it("reads .env from the given root", async () => {
    const { entry, outDir, streams } = await makeApp(
      `<script setup>\nconst msg = import.meta.env.VITE_FROM_ROOT;\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );
    const root = path.join(path.dirname(entry), "src");
    await mkdir(root);
    await writeFile(path.join(root, ".env"), "VITE_FROM_ROOT=env-in-root\n");

    const output = await build({ entry, root, outDir, ...streams });

    expect(await readFile(output.entryFile, "utf8")).toContain("env-in-root");
  }, 20000);

  it("rejects a <style> block, naming the file that used it", async () => {
    const { entry, outDir, streams } = await makeApp(
      dedent`
        <script setup>
        const msg = "hello";
        </script>
        <template><div>{{ msg }}</div></template>
        <style scoped>
        div { color: red; }
        </style>
      `,
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
