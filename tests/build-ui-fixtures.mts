#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Builds every `tests/ui-fixtures/*.vue` into `<outDir>/<name>.js`, one
// self-contained bundle each, in a single process. Exits non-zero and names
// the fixture on the first failure. Usage: `node build-ui-fixtures.mjs <outDir>`.

import { copyFile, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { createViteBundler } from "../packages/cli/src/adapter/vite/index.mts";
import * as adapterCore from "../packages/cli/src/adapterCore.mts";

const outDir = process.argv[2];
if (!outDir) {
  console.error("usage: build-ui-fixtures.mts <outDir>");
  process.exit(2);
}

const fixturesDir = path.join(import.meta.dirname, "tests/ui-fixtures");
// Under this package so the entries resolve `incajs/vue` from its node_modules.
const entriesRoot = path.join(import.meta.dirname, "node_modules/.inca-ui");
const bundler = createViteBundler(adapterCore);
const sink = new Writable({
  write(_chunk, _encoding, callback) {
    callback();
  },
});

await mkdir(entriesRoot, { recursive: true });
const entriesDir = await mkdtemp(path.join(entriesRoot, "run-"));
await mkdir(outDir, { recursive: true });
const names = (await readdir(fixturesDir))
  .filter((file) => file.endsWith(".vue"))
  .map((file) => path.basename(file, ".vue"));

let failed = false;
for (const name of names) {
  const entry = path.join(entriesDir, `${name}.mts`);
  const buildDir = path.join(outDir, `${name}.build`);
  const app = JSON.stringify(path.join(fixturesDir, `${name}.vue`));
  await writeFile(
    entry,
    `import { createIncaApp } from "incajs/vue";\nimport App from ${app};\ncreateIncaApp(App).mount();\n`,
  );
  try {
    const output = await bundler.build({
      entry,
      outDir: buildDir,
      stdout: sink,
      stderr: sink,
      quiet: true,
    });
    await copyFile(output.entryFile, path.join(outDir, `${name}.js`));
    await rm(buildDir, { recursive: true, force: true });
  } catch (error) {
    console.error(
      `fixture ${name}.vue failed to build: ${error instanceof Error ? error.message : String(error)}`,
    );
    failed = true;
    break;
  }
}
await rm(entriesDir, { recursive: true, force: true });
if (failed) process.exit(1);
