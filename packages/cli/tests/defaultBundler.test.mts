// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { afterAll, beforeAll, expect, it } from "vitest";

import { defaultBundler } from "../src/defaultBundler.mts";

const scratchRoot = path.join(import.meta.dirname, "tmp", "default-bundler");

beforeAll(() => mkdir(scratchRoot, { recursive: true }));
afterAll(() => rm(scratchRoot, { recursive: true, force: true }));

it("builds templates against incajs/vue/runtime", async () => {
  const appDir = await mkdtemp(path.join(scratchRoot, "app-"));
  const stub = path.join(appDir, "node_modules/incajs");
  await mkdir(stub, { recursive: true });
  await writeFile(
    path.join(stub, "package.json"),
    JSON.stringify({
      name: "incajs",
      type: "module",
      exports: { "./vue/runtime": "./runtime.mjs" },
    }),
  );
  await writeFile(
    path.join(stub, "runtime.mjs"),
    `export * from "@vue/runtime-core";\nexport const withModifiers = (fn) => { "DEFAULT_RUNTIME_MARKER"; return fn; };\n`,
  );
  await writeFile(
    path.join(appDir, "App.vue"),
    `<template><div @click.stop="1">x</div></template>\n`,
  );
  const entry = path.join(appDir, "entry.mts");
  await writeFile(entry, `import App from "./App.vue";\nexport default App;\n`);
  const sink = new Writable({ write: (_chunk, _encoding, callback) => callback() });

  const output = await defaultBundler.build({
    entry,
    outDir: path.join(appDir, "dist"),
    stdout: sink,
    stderr: sink,
    quiet: true,
  });

  expect(await readFile(output.entryFile, "utf8")).toContain("DEFAULT_RUNTIME_MARKER");
}, 20000);
