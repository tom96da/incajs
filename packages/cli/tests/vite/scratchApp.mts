// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

export interface ScratchApp {
  entry: string;
  outDir: string;
  vuePath: string;
  /** Spread into a build call to keep the bundler's output off the test console. */
  streams: { stdout: Writable; stderr: Writable; quiet: true };
  /** Everything Vite wrote to {@link ScratchApp.streams}. */
  logs: () => string;
}

function sink(): { stream: Writable; text: () => string } {
  const chunks: string[] = [];
  return {
    stream: new Writable({
      write(chunk, _encoding, callback) {
        chunks.push(String(chunk));
        callback();
      },
    }),
    text: () => chunks.join(""),
  };
}

const RUNTIME_PACKAGE = "scratch-runtime";

/** The module name the scratch apps' templates import their helpers from. */
export const TEST_RUNTIME_MODULE = `${RUNTIME_PACKAGE}/core`;

// Stands in for the modifier helpers; the marker shows up in a bundle that imported them.
const runtimeModule = `export * from "@vue/runtime-core";
export const withModifiers = (fn) => { "STUB_WITH_MODIFIERS"; return fn; };
export const withKeys = (fn) => { "STUB_WITH_KEYS"; return fn; };
`;

/**
 * Makes `TEST_RUNTIME_MODULE`, the module compiled templates import,
 * resolve under `scratchRoot` to a stub.
 */
async function writeRuntimeModule(scratchRoot: string): Promise<void> {
  const dir = path.join(scratchRoot, "node_modules", RUNTIME_PACKAGE);
  await mkdir(dir, { recursive: true });
  await writeFile(
    path.join(dir, "package.json"),
    JSON.stringify({
      name: RUNTIME_PACKAGE,
      type: "module",
      exports: { [TEST_RUNTIME_MODULE.replace(RUNTIME_PACKAGE, ".")]: "./core.mjs" },
    }),
  );
  await writeFile(path.join(dir, "core.mjs"), runtimeModule);
}

/**
 * A scratch-app scaffold scoped to its own directory under `tests/tmp/`
 * (gitignored) rather than a real OS tmpdir: this mirrors how Vite
 * resolves a real app's `@vue/runtime-core` import, by walking up to this
 * package's own `node_modules`. Each caller gets its own `name` so
 * concurrently running test files never race on the same directory.
 */
export function scratchApp(name: string): {
  setUp: () => Promise<void>;
  tearDown: () => Promise<void>;
  makeApp: (vueSource: string) => Promise<ScratchApp>;
} {
  const scratchRoot = path.join(import.meta.dirname, "tmp", name);

  return {
    setUp: async () => {
      await mkdir(scratchRoot, { recursive: true });
      await writeRuntimeModule(scratchRoot);
    },
    tearDown: () => rm(scratchRoot, { recursive: true, force: true }),
    async makeApp(vueSource: string): Promise<ScratchApp> {
      const appDir = await mkdtemp(path.join(scratchRoot, "app-"));
      const vuePath = path.join(appDir, "App.vue");
      const entry = path.join(appDir, "entry.mts");
      await writeFile(vuePath, vueSource);
      await writeFile(entry, `import App from "./App.vue";\nexport default App;\n`);
      const out = sink();
      const err = sink();
      return {
        entry,
        outDir: path.join(appDir, "dist"),
        vuePath,
        streams: { stdout: out.stream, stderr: err.stream, quiet: true },
        logs: () => out.text() + err.text(),
      };
    },
  };
}
