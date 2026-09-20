// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readFile, writeFile } from "node:fs/promises";

import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";

import { watch } from "../../../src/adapter/vite/index.mts";
import { scratchApp } from "./scratchApp.mts";
import type { BuildOutput, Watcher } from "../../../src/adapter/types.mts";

const { setUp, tearDown, makeApp } = scratchApp("watch");

let watchers: Watcher[] = [];

beforeAll(setUp);
afterAll(tearDown);
afterEach(async () => {
  await Promise.all(watchers.map((watcher) => watcher.close()));
  watchers = [];
});

describe("watch", () => {
  it("compiles a .vue file, reporting the entry it wrote", async () => {
    const { entry, outDir, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const output = await new Promise<BuildOutput>((resolve, reject) => {
      watch({
        entry,
        outDir,
        mode: "development",
        ...streams,
        onBuild: resolve,
        onError: (error) => reject(new Error(error.message)),
      })
        .then((w) => watchers.push(w))
        .catch(reject);
    });

    expect(output.outDir).toBe(outDir);
    const bundle = await readFile(output.entryFile, "utf8");
    expect(bundle).toContain("@vue/runtime-core");
    expect(bundle).not.toMatch(/from\s+["']vue["']/);
  }, 20000);

  it("rebuilds when the .vue file changes", async () => {
    const { entry, outDir, vuePath, streams } = await makeApp(
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    let builds = 0;
    let resolveBuild!: (output: BuildOutput) => void;
    let nextBuild = new Promise<BuildOutput>((resolve) => {
      resolveBuild = resolve;
    });

    const watcher = await watch({
      entry,
      outDir,
      mode: "development",
      ...streams,
      onBuild: (output) => {
        builds += 1;
        resolveBuild(output);
      },
      onError: (error) => {
        throw new Error(error.message);
      },
    });
    watchers.push(watcher);

    await nextBuild;
    nextBuild = new Promise((resolve) => {
      resolveBuild = resolve;
    });

    await writeFile(
      vuePath,
      `<script setup>\nconst msg = "second";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );
    const output = await nextBuild;

    expect(builds).toBe(2);
    expect(output.changed?.file).toBe(vuePath);
    const bundle = await readFile(output.entryFile, "utf8");
    expect(bundle).toContain("second");
  }, 20000);

  it("reports a syntax error without throwing", async () => {
    const { entry, outDir, streams } = await makeApp(
      `<script setup>\nconst broken = ;\n</script>\n<template><div/></template>\n`,
    );

    const error = await new Promise<{ message: string; stack: string | null }>(
      (resolve, reject) => {
        watch({
          entry,
          outDir,
          mode: "development",
          ...streams,
          onBuild: () => reject(new Error("expected a build error, got a successful build")),
          onError: resolve,
        })
          .then((watcher) => watchers.push(watcher))
          .catch(reject);
      },
    );

    expect(error.message).toBeTruthy();
  }, 20000);
});
