// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { DevEnvironment } from "vite";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { HotPayload } from "vite";

import { fsLag } from "../support/fsLag.mts";
import { connectRunner, discard, useHmr } from "./hmrApp.mts";
import type { HmrOptions } from "../../src/types.mts";

const { hmr, makeApp, channels } = useHmr("hmr-warm-up");

describe("entry warm-up", () => {
  const APP = `<script setup>\nconst msg = "first";\n</script>\n<template><div>{{ msg }}</div></template>\n`;

  /** Records the entry transforms and whether each had settled when `hmr()` resolved. */
  function watchTransforms() {
    const calls: { url: string; settled: boolean }[] = [];
    const environments: DevEnvironment[] = [];
    const original = Reflect.get(
      DevEnvironment.prototype,
      "transformRequest",
    ) as DevEnvironment["transformRequest"];
    vi.spyOn(DevEnvironment.prototype, "transformRequest").mockImplementation(function (
      this: DevEnvironment,
      url,
    ) {
      const call = { url, settled: false };
      calls.push(call);
      environments.push(this);
      const result = original.call(this, url);
      void result.then(
        () => (call.settled = true),
        () => (call.settled = true),
      );
      return result;
    });
    return Object.assign(calls, { environments });
  }

  afterEach(() => vi.restoreAllMocks());

  function options(entry: string, streams: object, extra: Partial<HmrOptions> = {}) {
    return {
      entry,
      cwd: path.dirname(entry),
      stdout: discard(),
      stderr: discard(),
      notify: () => {},
      reload: () => {},
      onError: () => {},
      ...streams,
      ...extra,
    } as HmrOptions;
  }

  it("transforms the entry once, before hmr() resolves", async () => {
    const { entry, streams } = await makeApp(APP);
    const calls = watchTransforms();

    channels.push(await hmr(options(entry, streams)));

    expect(calls.map(({ url, settled }) => ({ url, settled }))).toEqual([
      { url: entry, settled: true },
    ]);
    const module = await calls.environments[0]?.moduleGraph.getModuleByUrl(entry);
    expect(module?.transformResult).toBeTruthy();
  }, 20000);

  it("resolves when the entry has a syntax error of its own", async () => {
    const { entry, streams } = await makeApp(APP);
    await writeFile(entry, "export default (;\n");
    const errors: unknown[] = [];
    const calls = watchTransforms();

    channels.push(await hmr(options(entry, streams, { onError: (e) => errors.push(e) })));

    expect(errors).toEqual([]);
    const module = await calls.environments[0]?.moduleGraph.getModuleByUrl(entry);
    expect(module?.transformResult ?? null).toBeNull();
  }, 20000);

  it("fetches a failed entry again with the same message", async () => {
    const { entry, streams } = await makeApp(APP);
    await writeFile(entry, "export default (;\n");
    const warmUpErrors: unknown[] = [];
    const original = Reflect.get(
      DevEnvironment.prototype,
      "transformRequest",
    ) as DevEnvironment["transformRequest"];
    vi.spyOn(DevEnvironment.prototype, "transformRequest").mockImplementation(async function (
      this: DevEnvironment,
      url,
    ) {
      try {
        return await original.call(this, url);
      } catch (error) {
        warmUpErrors.push(error);
        throw error;
      }
    });
    let client: ReturnType<typeof connectRunner>;
    const channel = await hmr(
      options(entry, streams, { notify: (p) => client.notify(p as HotPayload) }),
    );
    channels.push(channel);
    client = connectRunner(channel.dispatch);

    expect(warmUpErrors).toHaveLength(1);
    await expect(client.runner.import(entry)).rejects.toThrow(
      (warmUpErrors[0] as Error).message.split("\n")[0],
    );
  }, 20000);

  it("prints nothing when an import of the entry is broken", async () => {
    const { entry, streams } = await makeApp(APP);
    await writeFile(path.join(path.dirname(entry), "broken.mts"), "export default (;\n");
    await writeFile(entry, `import "./broken.mts";\nexport default 1;\n`);
    const chunks: string[] = [];
    const sink = new Writable({
      write(chunk, _encoding, callback) {
        chunks.push(String(chunk));
        callback();
      },
    });
    const calls = watchTransforms();

    channels.push(await hmr(options(entry, streams, { stdout: sink, stderr: sink, quiet: false })));

    expect(calls[0]).toEqual({ url: entry, settled: true });
    expect(chunks.join("")).toBe("");
  }, 20000);

  it("emits file-changed for a save after hmr() resolves", async () => {
    const { entry, vuePath, streams } = await makeApp(APP);
    const notified: unknown[] = [];
    let client: ReturnType<typeof connectRunner>;
    const channel = await hmr(
      options(entry, streams, {
        notify: (p) => {
          notified.push(p);
          client.notify(p as HotPayload);
        },
      }),
    );
    channels.push(channel);
    client = connectRunner(channel.dispatch);
    await fsLag();

    await writeFile(vuePath, APP.replace("{{ msg }}", "saved {{ msg }}"));

    await vi.waitFor(
      () =>
        expect(notified).toContainEqual({
          type: "custom",
          event: "file-changed",
          data: { file: vuePath },
        }),
      { timeout: 15000 },
    );
    const app = (await client.runner.import(vuePath)) as { default: { render: () => void } };
    expect(String(app.default.render)).toContain("saved ");
  }, 20000);
});
