// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import vue from "@vitejs/plugin-vue";
import dedent from "ts-dedent";

import type { ViteIntegration } from "../../src/vite/index.mts";

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

/**
 * Writes the fake frontend package under `appDir/node_modules`, listing the peers
 * each frontend in `frontends` needs. Every scratch app gets one without peers.
 */
export async function writeIncajs(
  appDir: string,
  frontends: Record<string, string[]> = {},
  pkg = "incajs",
): Promise<void> {
  const dir = path.join(appDir, "node_modules", pkg);
  await mkdir(dir, { recursive: true });
  await writeFile(
    path.join(dir, "package.json"),
    JSON.stringify({
      name: pkg,
      exports: { ".": "./index.mjs", "./style": "./style.mjs" },
      incajs: {
        frontends: Object.fromEntries(
          Object.entries(frontends).map(([name, peers]) => [name, { peers }]),
        ),
      },
    }),
  );
  // `vue` calls document.createElement when it loads.
  await writeFile(
    path.join(dir, "index.mjs"),
    "export const document = { createElement: () => ({}) };\n",
  );
  await writeFile(
    path.join(dir, "style.mjs"),
    dedent`
      export function updateStyle(key, css) {
        console.debug(key, css);
      }
      export function removeStyle() {}
    `,
  );
}

/** A Vue integration for the scratch apps, which resolve `incajs` to a stub. */
export const TEST_INTEGRATION: ViteIntegration = {
  frontend: ["incajs", "vue"],
  plugins: () => [
    vue({
      template: {
        // Hoisted static content becomes createStaticVNode, which needs insertStaticContent.
        compilerOptions: { hoistStatic: false },
      },
    }),
  ],
  unsupported: [],
  inject: { document: ["incajs", "document"] },
  styleModule: "incajs/style",
};

/**
 * A scratch-app scaffold scoped to its own directory under `tests/tmp/`
 * (gitignored). A bare `vue` import in a template resolves by walking up to this
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
    },
    tearDown: () => rm(scratchRoot, { recursive: true, force: true }),
    async makeApp(vueSource: string): Promise<ScratchApp> {
      const appDir = await mkdtemp(path.join(scratchRoot, "app-"));
      const vuePath = path.join(appDir, "App.vue");
      const entry = path.join(appDir, "entry.mts");
      await writeFile(vuePath, vueSource);
      await writeFile(entry, `import App from "./App.vue";\nexport default App;\n`);
      await writeIncajs(appDir);
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
