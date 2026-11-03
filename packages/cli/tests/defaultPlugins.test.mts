// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import dedent from "ts-dedent";
import { afterAll, beforeAll, expect, it } from "vitest";

import { defaultBundler } from "../src/defaultPlugins.mts";

const scratchRoot = path.join(import.meta.dirname, "tmp", "default-bundler");

beforeAll(() => mkdir(scratchRoot, { recursive: true }));
afterAll(() => rm(scratchRoot, { recursive: true, force: true }));

/**
 * Builds an app whose template is `template` through `defaultBundler`, with `incajs/vue/runtime`
 * stubbed as the Vue core plus `runtimeExtra`.
 * @returns the text of the bundle
 */
async function bundleOf(template: string, runtimeExtra: string): Promise<string> {
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
    `export * from "@vue/runtime-core";\n${runtimeExtra}`,
  );
  await writeFile(path.join(appDir, "App.vue"), `<template>${template}</template>\n`);
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

  return readFile(output.entryFile, "utf8");
}

it("builds templates against incajs/vue/runtime", async () => {
  const bundle = await bundleOf(
    `<div @click.stop="1">x</div>`,
    `export const withModifiers = (fn) => { "DEFAULT_RUNTIME_MARKER"; return fn; };\n`,
  );

  expect(bundle).toContain("DEFAULT_RUNTIME_MARKER");
}, 20000);

it("compiles many static elements without createStaticVNode", async () => {
  const bundle = await bundleOf(
    "<i>x</i>".repeat(25),
    `export const createStaticVNode = (...args) => { "STATIC_MARKER"; return args; };\n`,
  );

  expect(bundle).not.toContain("STATIC_MARKER");
}, 20000);

it("checks the peers that incajs lists for vue", async () => {
  const appDir = await mkdtemp(path.join(scratchRoot, "app-"));
  const stub = path.join(appDir, "node_modules/incajs");
  await mkdir(stub, { recursive: true });
  await writeFile(
    path.join(stub, "package.json"),
    JSON.stringify({
      name: "incajs",
      type: "module",
      exports: { "./vue/runtime": "./runtime.mjs" },
      incajs: { frontends: { vue: { peers: ["@vue/runtime-dom"] } } },
    }),
  );
  await writeFile(path.join(stub, "runtime.mjs"), `export * from "@vue/runtime-core";\n`);
  await writeFile(
    path.join(appDir, "App.vue"),
    dedent`
      <script setup>
      import "@vue/runtime-dom";
      </script>
      <template><div /></template>
    `,
  );
  const entry = path.join(appDir, "entry.mts");
  await writeFile(entry, `import App from "./App.vue";\nexport default App;\n`);
  const sink = new Writable({ write: (_chunk, _encoding, callback) => callback() });

  await expect(
    defaultBundler.build({
      entry,
      outDir: path.join(appDir, "dist"),
      stdout: sink,
      stderr: sink,
      quiet: true,
    }),
  ).rejects.toThrow("ERR_INCA_PEER_NOT_FOUND");
}, 20000);
