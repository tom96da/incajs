// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import { dev } from "../src/dev.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import {
  waitFor,
  mockHost,
  slowReadyMockHost,
  hmrMockHost,
  makeUnspawnableHost,
  makeSink,
  makeFakeHmrBundler,
} from "./support/dev.mts";

describe("dev with --experimental-hmr", () => {
  const scratch = scratchConfigApp("dev-hmr");
  beforeAll(scratch.setUp);
  afterAll(scratch.tearDown);

  it("roundtrips a vite payload the app sends back through the dev channel", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const stdout = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: hmrMockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    bundler.emitVite({ type: "custom", event: "greet", data: { hi: 1 } });
    await waitFor(() =>
      expect(bundler.dispatched).toEqual([{ type: "custom", event: "greet", data: { hi: 1 } }]),
    );

    controller.abort();
    await running;
  });

  it("turns a full-reload payload into the reload RPC, not a vite notification", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: slowReadyMockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    bundler.emitFullReload();
    await waitFor(() => expect(stderr.text()).toContain("reload #1"));

    expect(bundler.dispatched).toEqual([]);

    controller.abort();
    await running;
  });

  it("prints a failed update the way a failed build is printed", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: mockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    bundler.emitError({ message: "bad template", stack: "at somewhere" });
    await waitFor(() => expect(stderr.text()).toContain("[inca] build failed"));

    controller.abort();
    await running;

    expect(stderr.text()).toContain("[inca] build failed: bad template");
    expect(stderr.text()).toContain("at somewhere");
  });

  it("prints an update only once the host is ready", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const hmr = vi.spyOn(bundler, "hmr");
    const stdout = makeSink();
    const controller = new AbortController();

    const cwd = await scratch.makeApp({});
    const running = dev({
      cwd,
      entry: "unused",
      bundler,
      hostBin: slowReadyMockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    // The mock host sends `ready` 200 ms after it starts.
    await waitFor(() => expect(hmr).toHaveBeenCalled());
    bundler.emitUpdate({ file: path.join(cwd, "src/early.vue"), took: 1 });
    expect(stdout.text()).not.toContain("update");

    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));
    bundler.emitUpdate({ file: path.join(cwd, "src/late.vue"), took: 2 });

    controller.abort();
    await running;

    expect(stdout.text()).toMatch(/\[inca\] update src\/late\.vue \(2ms\)/);
    expect(stdout.text()).not.toContain("early.vue");
  });

  it("reloads once after ready for a full reload that arrives before it", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const hmr = vi.spyOn(bundler, "hmr");
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: slowReadyMockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    // The mock host sends `ready` 200 ms after it starts.
    await waitFor(() => expect(hmr).toHaveBeenCalled());
    bundler.emitFullReload();
    await waitFor(() => expect(stderr.text()).toContain("reload #1"));
    expect(stdout.text()).toContain("[inca] ready");

    controller.abort();
    await running;

    expect(stderr.text()).not.toContain("reload #2");
    expect(stderr.text()).not.toContain("reload failed");
  });

  it("folds the full reloads that arrive before ready into one", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const hmr = vi.spyOn(bundler, "hmr");
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: slowReadyMockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await waitFor(() => expect(hmr).toHaveBeenCalled());
    bundler.emitFullReload();
    bundler.emitFullReload();
    await waitFor(() => expect(stderr.text()).toContain("reload #1"));

    bundler.emitFullReload();
    await waitFor(() => expect(stderr.text()).toContain("reload #2"));

    controller.abort();
    await running;

    expect(stderr.text()).not.toContain("reload #3");
  });

  it("stops instead of hanging when the host fails to start", async () => {
    const hostBin = await makeUnspawnableHost(await scratch.makeApp({}));
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    // HMR mode has no rebuild loop to retry a failed start on, so dev()
    // itself must resolve rather than wait forever for a signal that
    // will never come.
    await running;
    expect(stderr.text()).toContain("failed to start the host");

    expect(stdout.text()).not.toContain("[inca] ready");
  });
});
