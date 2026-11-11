// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { mkdir, mkdtemp, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { Writable } from "node:stream";
import { setTimeout } from "node:timers/promises";
import { stripVTControlCharacters } from "node:util";

import dedent from "ts-dedent";
import { describe, expect, it, vi } from "vitest";
import type { HotPayload } from "vite";

import * as adapterCore from "../../src/adapterCore.mts";
import { createViteBundler } from "../../src/vite/index.mts";
import { fsLag } from "../support/fsLag.mts";
import { connectRunner, discard, useHmr } from "./hmrApp.mts";
import { TEST_INTEGRATION, writeIncajs } from "./scratchApp.mts";
import type { HmrOptions } from "../../src/types.mts";

const { hmr, makeApp, channels, builtRuntimePath } = useHmr("hmr");
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

  it("binds document in a dependency that sits outside the app", async () => {
    const outside = await realpath(await mkdtemp(path.join(tmpdir(), "inca-dependency-")));
    const dependency = path.join(outside, "dependency.mjs");
    await writeFile(dependency, "globalThis.title = document.title;\n");
    const { entry, streams } = await makeApp(
      `<script setup>\nimport ${JSON.stringify(dependency)};\n</script>\n<template><div /></template>\n`,
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
    await rm(outside, { recursive: true, force: true });

    const ids = [...client.runner.evaluatedModules.idToModuleMap.keys()];
    expect(ids.some((id) => id.endsWith("/node_modules/incajs/index.mjs"))).toBe(true);
  }, 20000);

  it("applies a stylesheet through the client shim", async () => {
    const { entry, streams } = await makeApp(
      `<script setup>\nimport "./a.css";\n</script>\n<template><div /></template>\n`,
    );
    const appDir = path.dirname(entry);
    await writeFile(path.join(appDir, "a.css"), ".a { width: 1px; }\n");
    await writeFile(
      path.join(appDir, "node_modules/incajs/style.mjs"),
      dedent`
        export function updateStyle(key, css) {
          globalThis.__hmrStyleCalls.push([key, css]);
        }
        export function removeStyle(key) {
          globalThis.__hmrStyleCalls.push([key]);
        }
      `,
    );
    const calls: [string, string?][] = [];
    Object.assign(globalThis, { __hmrStyleCalls: calls });
    const channel = await hmr({
      entry,
      cwd: appDir,
      ...streams,
      notify: (payload) => client.notify(payload as HotPayload),
      reload: () => {},
      onError: () => {},
    });
    channels.push(channel);
    const client = connectRunner(channel.dispatch);

    await client.runner.import(entry);

    expect(calls).toEqual([[expect.stringMatching(/\/a\.css$/), ".a { width: 1px; }\n"]]);
  }, 20000);

  it("removes the stylesheet of an import the edited module dropped", async () => {
    const { entry, vuePath, streams } = await makeApp(
      `<script setup>\nimport "./a.css";\n</script>\n<template><div /></template>\n`,
    );
    const appDir = path.dirname(entry);
    await writeFile(path.join(appDir, "a.css"), ".a { width: 1px; }\n");
    await writeFile(
      path.join(appDir, "node_modules/incajs/style.mjs"),
      dedent`
        export function updateStyle() {}
        export function removeStyle(key) {
          globalThis.__hmrStyleCalls.push([key]);
        }
      `,
    );
    const calls: [string, string?][] = [];
    Object.assign(globalThis, { __hmrStyleCalls: calls });
    const channel = await hmr({
      entry,
      cwd: appDir,
      ...streams,
      notify: (payload) => client.notify(payload as HotPayload),
      reload: () => {},
      onError: () => {},
    });
    channels.push(channel);
    const client = connectRunner(channel.dispatch);
    await client.runner.import(entry);

    await writeFile(vuePath, `<template><div /></template>\n`);

    await vi.waitFor(() => expect(calls).toEqual([[expect.stringMatching(/\/a\.css$/)]]), {
      timeout: 15000,
    });
  }, 20000);

  it("creates the plugins again for each session", async () => {
    const { entry, streams } = await makeApp(`<template><div /></template>\n`);
    let created = 0;
    const counting = () => {
      created++;
      return TEST_INTEGRATION.plugins();
    };
    const bundler = createViteBundler(adapterCore, { ...TEST_INTEGRATION, plugins: counting });
    const options: HmrOptions = {
      entry,
      cwd: path.dirname(entry),
      runtimePath: builtRuntimePath,
      ...streams,
      notify: () => {},
      reload: () => {},
      onError: () => {},
    };

    channels.push(await bundler.hmr!(options));
    channels.push(await bundler.hmr!(options));

    expect(created).toBe(2);
  }, 20000);

  it("rejects an import of a peer the app's incajs marks and the app lacks", async () => {
    const { entry, streams } = await makeApp(
      `<script setup>\nimport "@vue/runtime-dom";\n</script>\n<template><div /></template>\n`,
    );
    await writeIncajs(path.dirname(entry), { vue: ["@vue/runtime-dom"] });
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

    await expect(client.runner.import(entry)).rejects.toThrow(
      "ERR_INCA_PEER_NOT_FOUND: @vue/runtime-dom can't be resolved. add it to the app's dependencies",
    );
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

  it("writes the app's root relative to its own directory into inca.json", async () => {
    const { entry, streams } = await makeApp(
      `<script setup>\nconst msg = "hello";\n</script>\n<template><div>{{ msg }}</div></template>\n`,
    );
    const cwd = path.dirname(entry);

    channels.push(
      await hmr({
        entry,
        cwd,
        ...streams,
        notify: () => {},
        reload: () => {},
        onError: () => {},
        runtimeConfig: { name: "Demo" },
      }),
    );

    const written = await readFile(path.join(cwd, "node_modules/.inca/hmr/inca.json"), "utf8");
    expect(JSON.parse(written)).toEqual({ name: "Demo", root: "../../.." });
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
