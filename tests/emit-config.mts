#!/usr/bin/env node
// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Writes the `inca.json` a build would, with every member the TypeScript
// side knows about set. `Required` keeps it exhaustive: a new member fails
// to type-check here until it is filled in.

import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { CONFIG_FILE_NAME } from "../packages/cli/src/adapter/vite/emitConfig.mts";
import type { RuntimeConfig } from "../packages/cli/src/config/loader.mts";
import type { WindowConfig } from "../packages/cli/src/config/types.mts";

const window: Required<WindowConfig> = {
  width: 1024,
  height: 768,
  title: "Demo Window",
  resizable: true,
  minWidth: 320,
  minHeight: 240,
};

const config: Required<RuntimeConfig> = {
  name: "Demo",
  identifier: "org.inca.demo",
  window,
};

const outDir = process.argv[2];
if (!outDir) {
  process.stderr.write("usage: emit-config.mts <out-dir>\n");
  process.exit(1);
}

await mkdir(outDir, { recursive: true });
await writeFile(path.join(outDir, CONFIG_FILE_NAME), `${JSON.stringify(config, undefined, 2)}\n`);
