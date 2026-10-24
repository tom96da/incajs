// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";
import { setTimeout } from "node:timers/promises";
import { stripVTControlCharacters } from "node:util";

import { build } from "tsdown";
import { ESModulesEvaluator, ModuleRunner } from "vite/module-runner";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import type { HotPayload } from "vite";
import type { ModuleRunnerTransport } from "vite/module-runner";

import * as adapterCore from "../../src/adapterCore.mts";
import { createViteBundler } from "../../src/vite/index.mts";
import tsdownConfig from "../../tsdown.config.mts";
import { fsLag } from "../support/fsLag.mts";
import { scratchApp, TEST_RUNTIME_MODULE } from "./scratchApp.mts";
import type { HmrChannel, HmrOptions } from "../../src/types.mts";

const { setUp, tearDown, makeApp } = scratchApp("hmr");

// Builds vite-runtime.js into its own scratch dir, from the package's real
// tsdown.config.mts, leaving the package's dist/ untouched.
const runtimeScratchDir = path.join(import.meta.dirname, "tmp/vite-runtime-build");
const builtRuntimePath = path.join(runtimeScratchDir, "vite-runtime.js");

/** Wraps the real `hmr()`, defaulting `runtimePath` to this test's own scratch build. */
function hmr(options: HmrOptions): Promise<HmrChannel> {
  return createViteBundler(adapterCore, TEST_RUNTIME_MODULE).hmr!({
    runtimePath: builtRuntimePath,
    ...options,
  });
}

// The real tsdown.config.mts's own "vite-runtime" entry, reused here rather
// than re-typed so this can never silently drift from the real build.
const runtimeEntry = (tsdownConfig.entry as Record<string, string>)["vite-runtime"]!;

beforeAll(async () => {
  await setUp();
  await build({
    ...tsdownConfig,
    config: false,
    cwd: path.resolve(import.meta.dirname, "../.."),
    entry: { "vite-runtime": runtimeEntry },
    outDir: runtimeScratchDir,
    // A .js runtime entry needs no declarations, and they would re-typecheck the package.
    dts: false,
    logLevel: "silent",
  });
}, 120000);
afterAll(async () => {
  await tearDown();
  await rm(runtimeScratchDir, { recursive: true, force: true });
});

/** A stream that drops what it is given. */
function discard(): Writable {
  return new Writable({ write: (_chunk, _encoding, callback) => callback() });
}

let channels: HmrChannel[] = [];
afterEach(async () => {
  await Promise.all(channels.map((channel) => channel.close()));
  channels = [];
});

/**
 * Wires a real `vite/module-runner` `ModuleRunner` to an `HmrChannel`, the
 * same round trip `packages/cli/src/vite/runtime` drives from
 * inside a running app. `notify` plays the server's half, `dispatch` the
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
    const reloads: ({ file: string; at: number } | undefined)[] = [];

    await fsLag();
    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: (payload) => {
        notified.push(payload);
        client.notify(payload as HotPayload);
      },
      reload: (changed) => reloads.push(changed),
      onError: () => {},
    });
    channels.push(channel);
    await fsLag();

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

    await fsLag();
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
    await fsLag();

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
    // A broken edit's `update` payload arrives before its failure does. It
    // must never be reported as a success.
    expect(updates).toEqual([]);
  }, 20000);

  it("calls onUpdate with the changed file and how long a successful update took", async () => {
    const { entry, vuePath, streams } = await makeApp(
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const updates: { file: string; took: number }[] = [];

    await fsLag();
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
    await fsLag();

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

    await fsLag();
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
    await fsLag();

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
    const reloads: ({ file: string; at: number } | undefined)[] = [];

    await fsLag();
    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: (payload) => {
        notified.push(payload);
        client.notify(payload as HotPayload);
      },
      reload: (changed) => reloads.push(changed),
      onError: () => {},
    });
    channels.push(channel);
    await fsLag();

    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    // entry.mts itself has no `import.meta.hot.accept` boundary. A real
    // browser client would fall back to a full page reload here, so this
    // custom channel's `send` must turn that into `reload()` instead of
    // forwarding a `"full-reload"` payload to `notify`.
    await writeFile(entry, `import App from "./App.vue";\nexport default App;\n// touched\n`);

    await vi.waitFor(() => expect(reloads.length).toBeGreaterThan(0), { timeout: 15000 });

    expect(notified.some((p) => (p as { type?: string }).type === "update")).toBe(false);
    expect(logs()).toBe("");
  }, 20000);

  it("hands the edited file to reload", async () => {
    const { entry, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const reloads: ({ file: string; at: number } | undefined)[] = [];

    await fsLag();
    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: (payload) => client.notify(payload as HotPayload),
      reload: (changed) => reloads.push(changed),
      onError: () => {},
    });
    channels.push(channel);
    await fsLag();

    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    await writeFile(entry, `import App from "./App.vue";\nexport default App;\n// touched\n`);

    await vi.waitFor(() => expect(reloads.length).toBeGreaterThan(0), { timeout: 15000 });

    expect(reloads[0]?.file).toBe(entry);
  }, 20000);

  it("reports no update for the fetch that follows a full-reload", async () => {
    const { entry, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );

    const updates: unknown[] = [];
    let reloaded = false;

    await fsLag();
    const channel = await hmr({
      entry,
      cwd: path.dirname(entry),
      ...streams,
      notify: (payload) => client.notify(payload as HotPayload),
      reload: () => {
        reloaded = true;
      },
      onError: () => {},
      onUpdate: (info) => updates.push(info),
    });
    channels.push(channel);
    await fsLag();

    // A reload restarts the app, which connects with a runner of its own.
    let client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    await writeFile(entry, `import App from "./App.vue";\nexport default App;\n// touched\n`);
    await vi.waitFor(() => expect(reloaded).toBe(true), { timeout: 15000 });

    client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    expect(updates).toEqual([]);
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
    await fsLag();
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
    await fsLag();

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

  it("writes an entry importing the real built vite-runtime.js", async () => {
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
    expect(content).toMatch(/^await start\("[^"]+", \{.*\}\);$/m);

    const resolved = path.resolve(path.dirname(channel.entryFile), specifier!);
    expect(existsSync(resolved)).toBe(true);
  }, 20000);

  it("passes the dev server's define globals to start", async () => {
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
    const globals = JSON.parse(content.match(/^await start\("[^"]+", (.*)\);$/m)![1]!) as object;
    expect(globals).toMatchObject({ __VUE_OPTIONS_API__: true });
  }, 20000);

  it("rejects when the entry cannot be written", async () => {
    const { entry, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );
    const cwd = path.dirname(entry);
    await mkdir(path.join(cwd, "node_modules/.inca/hmr/entry.js"), { recursive: true });

    await expect(
      hmr({ entry, cwd, ...streams, notify: () => {}, reload: () => {}, onError: () => {} }),
    ).rejects.toMatchObject({ code: "EISDIR" });
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

    // A second session against the same app must start cleanly. Nothing
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

  it("reads .env from the given envDir", async () => {
    const { entry } = await makeApp(`<template><div /></template>\n`);
    const envDir = path.dirname(entry);
    const root = path.join(envDir, "src");
    await mkdir(root);
    await writeFile(path.join(envDir, ".env"), "VITE_FROM_ROOT=env-in-app\n");
    const envEntry = path.join(envDir, "env-entry.mts");
    await writeFile(envEntry, `export default import.meta.env.VITE_FROM_ROOT;\n`);

    await fsLag();
    const channel = await hmr({
      entry: envEntry,
      root,
      envDir,
      cwd: envDir,
      stdout: discard(),
      stderr: discard(),
      quiet: true,
      notify: (payload) => client.notify(payload as HotPayload),
      reload: () => {},
      onError: () => {},
    });
    channels.push(channel);

    const client = connectRunner(channel.dispatch);
    const module = await client.runner.import<{ default: string }>(envEntry);

    expect(module.default).toBe("env-in-app");
  }, 20000);

  it("serves public from the given publicDir", async () => {
    const { entry } = await makeApp(`<template><div /></template>\n`);
    const appDir = path.dirname(entry);
    const root = path.join(appDir, "src");
    const publicDir = path.join(appDir, "public");
    await mkdir(root);
    await mkdir(publicDir);
    await writeFile(path.join(publicDir, "x.txt"), "x");
    const publicEntry = path.join(appDir, "public-entry.mts");
    await writeFile(publicEntry, `import x from "/x.txt";\nexport default x;\n`);

    const channel = await hmr({
      entry: publicEntry,
      root,
      publicDir,
      cwd: appDir,
      stdout: discard(),
      stderr: discard(),
      quiet: true,
      notify: (payload) => client.notify(payload as HotPayload),
      reload: () => {},
      onError: () => {},
    });
    channels.push(channel);

    const client = connectRunner(channel.dispatch);
    const module = await client.runner.import<{ default: string }>(publicEntry);

    expect(module.default).toBe("/x.txt");
  }, 20000);

  it("names an edited file by its url under the given root", async () => {
    const { entry, vuePath } = await makeApp(
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );
    const inner = path.join(path.dirname(entry), "inner", "entry.mts");
    await mkdir(path.dirname(inner), { recursive: true });
    await writeFile(inner, `import App from ${JSON.stringify(vuePath)};\nexport default App;\n`);

    const chunks: string[] = [];
    const sink = new Writable({
      write(chunk, _encoding, callback) {
        chunks.push(String(chunk));
        callback();
      },
    });

    const notified: unknown[] = [];
    await fsLag();
    const channel = await hmr({
      entry: inner,
      root: path.dirname(vuePath),
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
    await fsLag();

    const client = connectRunner(channel.dispatch);
    await client.runner.import(inner);

    await writeFile(
      vuePath,
      `<script setup>\nconst msg = "first";\n</script>\n<template><div>changed {{ msg }}</div></template>\n`,
    );

    await vi.waitFor(
      () => expect(notified.some((p) => (p as { type?: string }).type === "update")).toBe(true),
      { timeout: 15000 },
    );

    expect(stripVTControlCharacters(chunks.join(""))).toContain("hmr update /App.vue");
  }, 20000);
});
