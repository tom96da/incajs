// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";
import { setTimeout } from "node:timers/promises";

import { build } from "vite";
import { ESModulesEvaluator, ModuleRunner } from "vite/module-runner";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import type { HotPayload } from "vite";
import type { ModuleRunnerTransport } from "vite/module-runner";

import { createViteBundler } from "../../../src/adapter/vite/index.mts";
import * as adapterCore from "../../../src/adapterCore.mts";
import packageViteConfig from "../../../vite.config.mts";
import { scratchApp, TEST_RUNTIME_MODULE } from "./scratchApp.mts";
import type { HmrChannel, HmrOptions } from "../../../src/adapter/types.mts";

const { setUp, tearDown, makeApp } = scratchApp("hmr");

// Builds hmr-runtime.js into its own scratch dir, from the package's real
// vite.config.mts — self-contained, no dependency on (or mutation of)
// the package's own dist/.
const runtimeScratchDir = path.join(import.meta.dirname, "tmp/hmr-runtime-build");
const builtRuntimePath = path.join(runtimeScratchDir, "hmr-runtime.js");

/** Wraps the real `hmr()`, defaulting `runtimePath` to this test's own scratch build. */
function hmr(options: HmrOptions): Promise<HmrChannel> {
  return createViteBundler(adapterCore, TEST_RUNTIME_MODULE).hmr!({
    runtimePath: builtRuntimePath,
    ...options,
  });
}

// The real vite.config.mts's own "hmr-runtime" entry, reused here rather
// than re-typed so this can never silently drift from the real build.
const runtimeEntry = (packageViteConfig.build!.lib as { entry: Record<string, string> }).entry[
  "hmr-runtime"
]!;

beforeAll(async () => {
  await setUp();
  await build({
    ...packageViteConfig,
    configFile: false,
    // packageViteConfig sets no logLevel/customLogger of its own (that's
    // only in the user-app-facing config).
    logLevel: "silent",
    // Skips the real config's dts plugin: irrelevant to a .js runtime entry
    // and it would otherwise re-typecheck the whole package on every run.
    plugins: [],
    build: {
      ...packageViteConfig.build,
      lib: { ...(packageViteConfig.build!.lib as object), entry: { "hmr-runtime": runtimeEntry } },
      outDir: runtimeScratchDir,
      emptyOutDir: true,
    },
  });
}, 120000);
afterAll(async () => {
  await tearDown();
  await rm(runtimeScratchDir, { recursive: true, force: true });
});

let channels: HmrChannel[] = [];
afterEach(async () => {
  await Promise.all(channels.map((channel) => channel.close()));
  channels = [];
});

/**
 * Wires a real `vite/module-runner` `ModuleRunner` to an `HmrChannel`, the
 * same round trip `packages/cli/src/adapter/vite/runtime` drives from
 * inside a running app — `notify` plays the server's half, `dispatch` the
 * client's.
 */
function connectRunner(dispatch: HmrChannel["dispatch"]): {
  runner: ModuleRunner;
  notify: (payload: HotPayload) => void;
} {
  let onMessage: ((data: HotPayload) => void) | undefined;
  const transport: ModuleRunnerTransport = {
    connect(handlers) {
      onMessage = handlers.onMessage;
    },
    send: (data) => dispatch(data),
  };
  const runner = new ModuleRunner({ transport, hmr: { logger: false } }, new ESModulesEvaluator());
  return { runner, notify: (payload) => onMessage?.(payload) };
}

describe("hmr", () => {
  it("sends a Vue HMR update (not a reload) for a template-only edit, quiet", async () => {
    const { entry, vuePath, streams, logs } = await makeApp(
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const notified: unknown[] = [];
    const reloads: number[] = [];

    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: (payload) => {
        notified.push(payload);
        client.notify(payload as HotPayload);
      },
      reload: () => reloads.push(Date.now()),
      onError: () => {},
    });
    channels.push(channel);

    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    await writeFile(
      vuePath,
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>changed {{ msg }}</div></template>\n`,
    );

    await vi.waitFor(
      () => expect(notified.some((p) => (p as { type?: string }).type === "update")).toBe(true),
      { timeout: 15000 },
    );

    expect(reloads).toEqual([]);
    // A custom logger bypasses `logLevel`, so `quiet` must suppress this itself.
    expect(logs()).toBe("");
  }, 20000);

  it("loads the modifier helpers a template imports from the runtime core module", async () => {
    const { entry, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div @click.stop="msg = 'x'">{{ msg }}</div></template>\n`,
    );

    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: (payload) => client.notify(payload as HotPayload),
      reload: () => {},
      onError: () => {},
    });
    channels.push(channel);

    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    const modules = client.runner.evaluatedModules;
    const runtimeId = [...modules.idToModuleMap.keys()].find((id) => id.endsWith("/core.mjs"));
    expect(runtimeId).toBeDefined();
    const exports = modules.getModuleById(runtimeId!)?.exports as { withModifiers: () => unknown };
    expect(String(exports.withModifiers)).toContain("STUB_WITH_MODIFIERS");
  }, 20000);

  it("calls onError with a failed update's detail, still relays it to the app", async () => {
    const { entry, vuePath, streams } = await makeApp(
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const notified: unknown[] = [];
    const errors: { message: string }[] = [];
    const updates: { file: string; took: number }[] = [];

    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: (payload) => {
        notified.push(payload);
        client.notify(payload as HotPayload);
      },
      reload: () => {},
      onError: (error) => errors.push(error),
      onUpdate: (info) => updates.push(info),
    });
    channels.push(channel);

    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    // A template syntax error: an HMR boundary exists (no full-reload), but
    // the update itself fails to compile.
    await writeFile(
      vuePath,
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }</div></template>\n`,
    );

    await vi.waitFor(() => expect(errors.length).toBeGreaterThan(0), { timeout: 15000 });

    expect(errors[0]!.message).toContain("Interpolation end sign was not found");
    // Still relayed: the app's own pending fetch/HMR call may be waiting on it.
    expect(notified.length).toBeGreaterThan(0);
    // A broken edit's `update` payload arrives before its failure does — it
    // must never be reported as a success.
    expect(updates).toEqual([]);
  }, 20000);

  it("calls onUpdate with the changed file and how long it took, on a successful update", async () => {
    const { entry, vuePath, streams } = await makeApp(
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const updates: { file: string; took: number }[] = [];

    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: (payload) => client.notify(payload as HotPayload),
      reload: () => {},
      onError: () => {},
      onUpdate: (info) => updates.push(info),
    });
    channels.push(channel);

    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    await writeFile(
      vuePath,
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>changed {{ msg }}</div></template>\n`,
    );

    await vi.waitFor(() => expect(updates.length).toBeGreaterThan(0), { timeout: 15000 });

    expect(updates[0]!.file).toBe(vuePath);
    expect(updates[0]!.took).toBeGreaterThanOrEqual(0);
  }, 20000);

  it("applies the last of two saves 15 ms apart", async () => {
    const { entry, vuePath, streams } = await makeApp(
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const errors: unknown[] = [];
    const updates: { file: string; took: number }[] = [];

    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: (payload) => client.notify(payload as HotPayload),
      reload: () => {},
      onError: (error) => errors.push(error),
      onUpdate: (info) => updates.push(info),
    });
    channels.push(channel);

    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    await writeFile(
      vuePath,
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }</div></template>\n`,
    );
    await setTimeout(15);
    await writeFile(
      vuePath,
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>fixed {{ msg }}</div></template>\n`,
    );

    await vi.waitFor(() => expect(updates.length).toBeGreaterThan(0), { timeout: 15000 });
  }, 20000);

  it("reloads instead of notifying when a change has no HMR boundary, quiet", async () => {
    const { entry, streams, logs } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const notified: unknown[] = [];
    const reloads: number[] = [];

    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: (payload) => {
        notified.push(payload);
        client.notify(payload as HotPayload);
      },
      reload: () => reloads.push(Date.now()),
      onError: () => {},
    });
    channels.push(channel);

    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    // entry.mts itself has no `import.meta.hot.accept` boundary — a real
    // browser client would fall back to a full page reload here, so this
    // custom channel's `send` must turn that into `reload()` instead of
    // forwarding a `"full-reload"` payload to `notify`.
    await writeFile(entry, `import App from "./App.vue";\nexport default App;\n// touched\n`);

    await vi.waitFor(() => expect(reloads.length).toBeGreaterThan(0), { timeout: 15000 });

    expect(notified.some((p) => (p as { type?: string }).type === "update")).toBe(false);
    expect(logs()).toBe("");
  }, 20000);

  it("routes a non-quiet session's HMR/reload logging into the given streams", async () => {
    const { entry, vuePath } = await makeApp(
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const chunks: string[] = [];
    const sink = new Writable({
      write(chunk, _encoding, callback) {
        chunks.push(String(chunk));
        callback();
      },
    });

    const notified: unknown[] = [];
    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      stdout: sink,
      stderr: sink,
      quiet: false,
      notify: (payload) => {
        notified.push(payload);
        client.notify(payload as HotPayload);
      },
      reload: () => {},
      onError: () => {},
    });
    channels.push(channel);

    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    await writeFile(
      vuePath,
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>changed {{ msg }}</div></template>\n`,
    );

    await vi.waitFor(
      () => expect(notified.some((p) => (p as { type?: string }).type === "update")).toBe(true),
      { timeout: 15000 },
    );

    // Opposite regression: non-quiet mode must still capture this, not swallow it.
    expect(chunks.join("")).toContain("hmr update");
  }, 20000);

  it("writes an entry importing the real built hmr-runtime.js", async () => {
    const { entry, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: () => {},
      reload: () => {},
      onError: () => {},
    });
    channels.push(channel);

    const content = await readFile(channel.entryFile, "utf8");
    const specifier = content.match(/from\s+"([^"]+)"/)?.[1];
    expect(specifier).toBeTruthy();
    expect(content).toMatch(/start\("[^"]+"\)\.catch\(/);

    const resolved = path.resolve(path.dirname(channel.entryFile), specifier!);
    expect(existsSync(resolved)).toBe(true);
  }, 20000);

  it("removes a stale inca.json when the session's config produces none", async () => {
    const { entry, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );
    const options = {
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: () => {},
      reload: () => {},
      onError: () => {},
    };
    const configPath = path.join(options.cwd, "node_modules/.inca/hmr/inca.json");

    await (await hmr({ ...options, runtimeConfig: { name: "Demo" } })).close();
    expect(existsSync(configPath)).toBe(true);

    channels.push(await hmr(options));
    expect(existsSync(configPath)).toBe(false);
  }, 20000);

  it("tears the dev server down on close, freeing it to start again", async () => {
    const { entry, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const first = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: () => {},
      reload: () => {},
      onError: () => {},
    });
    await first.close();

    // A second session against the same app must start cleanly — nothing
    // from the first server (middleware, watcher, port) lingers.
    const second = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: () => {},
      reload: () => {},
      onError: () => {},
    });
    channels.push(second);

    expect(existsSync(second.entryFile)).toBe(true);
  }, 20000);
});
