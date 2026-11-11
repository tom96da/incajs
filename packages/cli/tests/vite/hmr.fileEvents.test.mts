// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { writeFile } from "node:fs/promises";
import path from "node:path";

import { describe, expect, it, vi } from "vitest";
import type { HotPayload, Plugin } from "vite";

import { vueViteIntegration } from "../../src/defaultPlugins.mts";
import { fsLag } from "../support/fsLag.mts";
import { connectRunner, useHmr } from "./hmrApp.mts";
import { TEST_INTEGRATION, TEST_RUNTIME_MODULE } from "./scratchApp.mts";

const { hmr, makeApp, channels } = useHmr("hmr-file-events");

describe("hmr file events", () => {
  /** Starts a session on a scratch app whose entry is loaded and imports `App.vue`. */
  async function startSession({
    integration = TEST_INTEGRATION,
    beforeStart = async (_dir: string) => {},
  } = {}) {
    const vueSource = `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`;
    const entrySource = `import App from "./App.vue";\nexport default App;\n`;
    const { entry, vuePath, streams } = await makeApp(vueSource);
    await writeFile(entry, entrySource);
    await beforeStart(path.dirname(entry));
    const notified: HotPayload[] = [];
    const reloads: ({ file: string; at: number } | undefined)[] = [];
    // The files whose events reached the plugins, in order.
    const seen: string[] = [];
    const observed: typeof integration = {
      ...integration,
      plugins: () => [
        ...integration.plugins(),
        {
          name: "test:seen",
          applyToEnvironment: (environment) => environment.name === "client",
          hotUpdate(options) {
            seen.push(options.file);
          },
        } satisfies Plugin,
      ],
    };
    await fsLag();
    const channel = await hmr(
      {
        entry,
        cwd: path.dirname(entry),
        ...streams,
        notify: (payload) => {
          notified.push(payload as HotPayload);
          client.notify(payload as HotPayload);
        },
        reload: (changed) => reloads.push(changed),
        onError: () => {},
      },
      observed,
    );
    channels.push(channel);
    await fsLag();
    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);
    const updates = () => notified.filter((p) => p.type === "update");
    const fileChanges = () =>
      notified.filter((p) => p.type === "custom" && p.event === "file-changed");
    return {
      dir: path.dirname(entry),
      entry,
      vuePath,
      vueSource,
      entrySource,
      reloads,
      seen,
      runner: client.runner,
      updates,
      fileChanges,
    };
  }

  it("ignores a rewrite of the entry with the same content", async () => {
    const { entry, vuePath, vueSource, entrySource, reloads, seen, updates } = await startSession();

    await writeFile(entry, entrySource);
    await vi.waitFor(() => expect(seen).toEqual([entry]), { timeout: 15000 });
    // Signal: a real change of the same file, then of App.vue, shows the rewrite was handled.
    await writeFile(entry, `${entrySource}// touched\n`);
    await vi.waitFor(() => expect(reloads).toHaveLength(1), { timeout: 15000 });
    await writeFile(vuePath, vueSource.replace("{{ msg }}", "changed {{ msg }}"));
    await vi.waitFor(() => expect(updates()).toHaveLength(1), { timeout: 15000 });

    expect(reloads).toHaveLength(1);
  }, 20000);

  it("ignores a rewrite of a .vue file the app never imported", async () => {
    const { entry, vuePath, vueSource, entrySource, reloads, seen, updates, fileChanges } =
      await startSession({
        // The entry no longer imports App.vue.
        beforeStart: (dir) => writeFile(path.join(dir, "entry.mts"), `export default 1;\n`),
      });

    await writeFile(vuePath, vueSource);
    await vi.waitFor(() => expect(seen).toEqual([vuePath]), { timeout: 15000 });
    await writeFile(entry, `${entrySource}// touched\n`);
    await vi.waitFor(() => expect(reloads).toHaveLength(1), { timeout: 15000 });

    expect(updates()).toEqual([]);
    expect(fileChanges()).toHaveLength(1);
  }, 20000);

  it("updates for an edit outside the module graph when a plugin chose modules", async () => {
    let typeFile = "";
    const vuePathOf = (file: string) => path.join(path.dirname(file), "App.vue");
    const real = vueViteIntegration(TEST_RUNTIME_MODULE);
    // Like plugin-vue for a type file that `defineProps<Props>()` imports.
    const integration: typeof real = {
      ...real,
      plugins: () => [
        ...real.plugins(),
        {
          name: "test:type-file",
          hotUpdate(options) {
            if (options.file !== typeFile) return;
            return [...this.environment.moduleGraph.getModulesByFile(vuePathOf(typeFile))!];
          },
        } satisfies Plugin,
      ],
    };
    const { updates } = await startSession({
      integration,
      beforeStart: async (dir) => {
        typeFile = path.join(dir, "types.d.ts");
        await writeFile(typeFile, "export interface Props {}\n");
      },
    });

    await writeFile(typeFile, "export interface Props { a: 1 }\n");

    await vi.waitFor(() => expect(updates()).toHaveLength(1), { timeout: 15000 });
  }, 20000);

  it("passes an unchanged dependency file on", async () => {
    const manifest = "{}\n";
    const { dir, fileChanges } = await startSession({
      beforeStart: (dir) => writeFile(path.join(dir, "package.json"), manifest),
    });

    await writeFile(path.join(dir, "package.json"), manifest);

    await vi.waitFor(() => expect(fileChanges()).toHaveLength(1), { timeout: 15000 });
  }, 20000);

  it("applies a change that an identical rewrite overlaps", async () => {
    let entryFile = "";
    let open = () => {};
    const gate = new Promise<void>((resolve) => (open = resolve));
    let gated = false;
    const real = vueViteIntegration(TEST_RUNTIME_MODULE);
    const integration: typeof real = {
      ...real,
      plugins: () => [
        ...real.plugins(),
        {
          name: "test:gate",
          applyToEnvironment: (environment) => environment.name === "client",
          async hotUpdate(options) {
            if (options.file !== entryFile || gated) return;
            gated = true;
            await gate;
          },
        } satisfies Plugin,
      ],
    };
    const { entry, entrySource, reloads, seen } = await startSession({ integration });
    entryFile = entry;

    await writeFile(entry, `${entrySource}// touched\n`);
    await vi.waitFor(() => expect(gated).toBe(true), { timeout: 15000 });
    await writeFile(entry, `${entrySource}// touched\n`);
    await vi.waitFor(() => expect(seen).toEqual([entry]), { timeout: 15000 });
    open();

    await vi.waitFor(() => expect(reloads).toHaveLength(1), { timeout: 15000 });
  }, 20000);

  it("ignores a creation event for a file with the content already loaded", async () => {
    let lateFile = "";
    let imported = () => {};
    const loaded = new Promise<void>((resolve) => (imported = resolve));
    const real = vueViteIntegration(TEST_RUNTIME_MODULE);
    const integration: typeof real = {
      ...real,
      plugins: () => [
        ...real.plugins(),
        {
          name: "test:after-load",
          applyToEnvironment: (environment) => environment.name === "client",
          // Holds the event until the app has loaded the file, whichever the watcher reports first.
          hotUpdate: {
            order: "pre",
            async handler(options) {
              if (options.file === lateFile) await loaded;
            },
          },
        } satisfies Plugin,
      ],
    };
    const { dir, runner, seen, fileChanges } = await startSession({ integration });
    lateFile = path.join(dir, "late.mjs");

    await writeFile(lateFile, "export default 1;\n");
    await runner.import(lateFile);
    imported();
    await vi.waitFor(() => expect(seen).toEqual([lateFile]), { timeout: 15000 });
    await writeFile(lateFile, "export default 2;\n");
    await vi.waitFor(() => expect(seen).toEqual([lateFile, lateFile]), { timeout: 15000 });

    expect(fileChanges()).toHaveLength(1);
  }, 20000);
});
