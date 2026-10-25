// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, it, onTestFinished, vi } from "vitest";

import { dev } from "../src/dev.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import {
  waitFor,
  mockHost,
  slowReadyMockHost,
  reloadFailsMockHost,
  hmrMockHost,
  logMockHost,
  useHostLog,
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

  it("hands the HMR session dev.stackPaths", async () => {
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
    controller.abort();
    await running;

    expect(bundler.channels[0]!.options.runtimeConfig).toMatchObject({
      dev: { stackPaths: "relative" },
    });
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

  it("prints a reload line for the edit behind a full-reload payload", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();
    const cwd = await scratch.makeApp({});

    const running = dev({
      cwd,
      entry: "unused",
      bundler,
      hostBin: slowReadyMockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    bundler.emitFullReload({ file: path.join(cwd, "src/main.ts"), at: Date.now() });
    await waitFor(() => expect(stderr.text()).toContain("reload #1"));

    await waitFor(() => expect(stdout.text()).toMatch(/\[inca\] reload src\/main\.ts \(\d+ms\)/));

    controller.abort();
    await running;
  });

  it("prints no reload line when the reload of a full-reload payload fails", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();
    const cwd = await scratch.makeApp({});

    const running = dev({
      cwd,
      entry: "unused",
      bundler,
      hostBin: reloadFailsMockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    bundler.emitFullReload({ file: path.join(cwd, "src/main.ts"), at: Date.now() });
    await waitFor(() => expect(stderr.text()).toContain("[inca] reload failed: boom"));

    expect(stdout.text()).not.toContain("reload src");

    controller.abort();
    await running;
  });

  it("prints no reload line for a full-reload payload without an edit", async () => {
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

    expect(stdout.text()).not.toContain("reload src");

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

  it("passes src as the root of the entry synthesized for App.vue", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const hmr = vi.spyOn(bundler, "hmr");
    const controller = new AbortController();
    const cwd = await scratch.makeApp({});

    const running = dev({
      cwd,
      entry: path.join(cwd, "node_modules/.inca/entry.mts"),
      bundler,
      hostBin: hmrMockHost,
      experimentalHmr: true,
      stdout: makeSink().stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await waitFor(() => expect(hmr).toHaveBeenCalled());
    controller.abort();
    await running;

    expect(hmr.mock.calls[0]![0].root).toBe(path.join(cwd, "src"));
  });

  it("passes the app directory for .env and public to the hmr channel", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const hmr = vi.spyOn(bundler, "hmr");
    const controller = new AbortController();
    const cwd = await scratch.makeApp({});

    const running = dev({
      cwd,
      entry: path.join(cwd, "node_modules/.inca/entry.mts"),
      bundler,
      hostBin: hmrMockHost,
      experimentalHmr: true,
      stdout: makeSink().stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await waitFor(() => expect(hmr).toHaveBeenCalled());
    controller.abort();
    await running;

    expect(hmr.mock.calls[0]![0]).toMatchObject({
      envDir: cwd,
      publicDir: path.join(cwd, "public"),
    });
  });

  /** An app whose `src/App.vue` is the only entry candidate. */
  async function makeAppVue(): Promise<string> {
    const cwd = await scratch.makeApp({});
    await mkdir(path.join(cwd, "src"), { recursive: true });
    await writeFile(path.join(cwd, "src/App.vue"), "<template><div/></template>\n");
    return cwd;
  }

  /** Starts `dev()` with the HMR channel and waits until its first host is ready. */
  async function startAuto(cwd: string, hostBin = logMockHost) {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();
    const hostLog = await useHostLog(cwd);
    onTestFinished(() => controller.abort());
    const running = dev({
      cwd,
      bundler,
      hostBin,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });
    const readyCount = () => stdout.text().match(/\[inca\] ready/g)?.length ?? 0;
    await waitFor(() => expect(readyCount()).toBe(1));
    return {
      bundler,
      stdout,
      stderr,
      hostLog,
      running,
      readyCount,
      abort: () => controller.abort(),
    };
  }

  /** Replaces the first channel with a second one and waits until the host has reloaded. */
  async function replaceChannel(cwd: string) {
    const run = await startAuto(cwd);
    await writeFile(path.join(cwd, "src/main.mts"), "");
    await waitFor(() => expect(run.bundler.channels).toHaveLength(2));
    await waitFor(async () => expect(await run.hostLog.lines()).toContain("reload"));
    return run;
  }

  it("reloads the host through a new channel when the entry changes", async () => {
    const cwd = await makeAppVue();
    const run = await startAuto(cwd);

    await writeFile(path.join(cwd, "src/main.mts"), "");
    await waitFor(() => expect(run.bundler.channels).toHaveLength(2));

    const [old, next] = run.bundler.channels;
    expect(old!.closed).toBe(true);
    expect(next!.openBefore).toBe(0);
    expect(next!.options).toMatchObject({
      entry: path.join(cwd, "src/main.mts"),
      root: path.join(cwd, "src"),
    });
    await waitFor(async () =>
      expect(await run.hostLog.lines()).toEqual(["start /unused/entry.js", "reload"]),
    );

    run.abort();
    await run.running;
  });

  it("reloads once the host is ready when the entry changes before that", async () => {
    const cwd = await makeAppVue();
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();
    onTestFinished(() => controller.abort());
    const running = dev({
      cwd,
      bundler,
      hostBin: slowReadyMockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });
    await waitFor(() => expect(bundler.channels).toHaveLength(1));

    await writeFile(path.join(cwd, "src/main.mts"), "");
    await waitFor(() => expect(bundler.channels).toHaveLength(2));
    await waitFor(() => expect(stderr.text()).toContain("reload #1"));

    controller.abort();
    await running;
  });

  it("ignores the error callback of a replaced channel", async () => {
    const cwd = await makeAppVue();
    const run = await replaceChannel(cwd);

    run.bundler.channels[0]!.options.onError({ message: "stale failure", stack: null });
    run.bundler.emitError({ message: "current failure", stack: null });

    expect(run.stderr.text()).toContain("build failed: current failure");
    expect(run.stderr.text()).not.toContain("stale failure");

    run.abort();
    await run.running;
  });

  it("ignores the update callback of a replaced channel", async () => {
    const cwd = await makeAppVue();
    const run = await replaceChannel(cwd);

    run.bundler.channels[0]!.options.onUpdate?.({ file: path.join(cwd, "src/stale.vue"), took: 1 });
    run.bundler.emitUpdate({ file: path.join(cwd, "src/current.vue"), took: 2 });

    expect(run.stdout.text()).toMatch(/\[inca\] update src\/current\.vue \(2ms\)/);
    expect(run.stdout.text()).not.toContain("stale.vue");

    run.abort();
    await run.running;
  });

  it("ignores the reload callback of a replaced channel", async () => {
    const cwd = await makeAppVue();
    const run = await replaceChannel(cwd);

    run.bundler.channels[0]!.options.reload();
    run.bundler.emitFullReload({ file: path.join(cwd, "src/current.vue"), at: Date.now() });
    // The host answers in order, so this line comes after the reply to any earlier reload.
    await waitFor(() => expect(run.stdout.text()).toContain("reload src/current.vue"));

    // One reload for the switch and one for the current channel.
    expect((await run.hostLog.lines()).filter((line) => line === "reload")).toHaveLength(2);

    run.abort();
    await run.running;
  });

  it("ignores the notification callback of a replaced channel", async () => {
    const cwd = await makeAppVue();
    const run = await replaceChannel(cwd);

    run.bundler.channels[0]!.options.notify({ type: "custom", event: "stale" });
    run.bundler.emitVite({ type: "custom", event: "current" });

    // The host echoes each notification back in order, so a stale one would come first.
    await waitFor(() =>
      expect(run.bundler.channels[1]!.dispatched).toEqual([{ type: "custom", event: "current" }]),
    );
    expect(run.bundler.channels[0]!.dispatched).toEqual([]);

    run.abort();
    await run.running;
  });
});
