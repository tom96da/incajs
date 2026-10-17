// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { afterAll, beforeAll, describe, expect, it, onTestFinished, vi } from "vitest";

import { dev } from "../src/dev.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import {
  waitFor,
  mockHost,
  reloadFailsMockHost,
  appErrorMockHost,
  protocolMismatchMockHost,
  startupFailureMockHost,
  slowReadyMockHost,
  exitingMockHost,
  FAKE_OUTPUT,
  makeFakeBundler,
  pretendPlatform,
  makeUnspawnableHost,
  makeSink,
} from "./support/dev.mts";

describe("dev", () => {
  const scratch = scratchConfigApp("dev");
  beforeAll(scratch.setUp);
  afterAll(scratch.tearDown);

  it("starts the host and prints ready once the first build lands", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: mockHost,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    controller.abort();
    await running;

    expect(stdout.text()).toContain("[inca] ready");
  });

  it("reloads the host on a later build without restarting it", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: mockHost,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));
    bundler.emitBuild();

    controller.abort();
    await running;

    expect(stderr.text()).not.toContain("reload failed");
  });

  it("names the file that triggered a reload and how long it took", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const controller = new AbortController();

    const cwd = await scratch.makeApp({});
    const running = dev({
      cwd,
      entry: "unused",
      bundler,
      hostBin: mockHost,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));
    expect(stdout.text()).not.toContain("reload");

    const vuePath = path.join(cwd, "src/App.vue");
    bundler.emitBuild({ ...FAKE_OUTPUT, changed: { file: vuePath, at: Date.now() } });
    await waitFor(() => expect(stdout.text()).toContain("reload src/App.vue"));

    controller.abort();
    await running;

    expect(stdout.text()).toMatch(/\[inca\] reload src\/App\.vue \(\d+ms\)/);
  });

  it("rejects before watching and releases the lock when the host binary is missing", async () => {
    const bundler = makeFakeBundler();
    const watch = vi.spyOn(bundler, "watch");
    const options = {
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      stdout: makeSink().stream,
      stderr: makeSink().stream,
      signal: new AbortController().signal,
    };

    const missing = { ...options, hostBin: "/nonexistent/inca-host" };
    await expect(dev(missing)).rejects.toMatchObject({ code: "ERR_INCA_HOST_BIN_NOT_FOUND" });
    // The lock is free again, so a second attempt reaches the same error.
    await expect(dev(missing)).rejects.toMatchObject({ code: "ERR_INCA_HOST_BIN_NOT_FOUND" });
    expect(watch).not.toHaveBeenCalled();
  });

  it("rejects with ERR_INCA_HOST_BIN_UNRESOLVED when no host binary resolves", async () => {
    // No platform package exists for win32, so nothing resolves.
    pretendPlatform("win32");
    vi.stubEnv("INCA_HOST_BIN", "");
    onTestFinished(() => void vi.unstubAllEnvs());
    const bundler = makeFakeBundler();
    const watch = vi.spyOn(bundler, "watch");

    await expect(
      dev({
        cwd: await scratch.makeApp({}),
        entry: "unused",
        bundler,
        stdout: makeSink().stream,
        stderr: makeSink().stream,
        signal: new AbortController().signal,
      }),
    ).rejects.toMatchObject({
      code: "ERR_INCA_HOST_BIN_UNRESOLVED",
      message: expect.stringMatching(/INCA_HOST_BIN/),
    });
    expect(watch).not.toHaveBeenCalled();
  });

  it("prints a build failure without ever starting the host", async () => {
    const hostBin = await makeUnspawnableHost(await scratch.makeApp({}));
    const bundler = makeFakeBundler();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      // A start attempt would print "failed to start".
      hostBin,
      stdout: makeSink().stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitError({ message: "syntax error", stack: "at somewhere" });
    await waitFor(() => expect(stderr.text()).toContain("[inca] build failed"));

    controller.abort();
    await running;

    expect(stderr.text()).toContain("[inca] build failed: syntax error");
    expect(stderr.text()).toContain("at somewhere");
    expect(stderr.text()).not.toContain("failed to start");
  });

  it("prints a failed reload (-32000) without throwing", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: reloadFailsMockHost,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));
    bundler.emitBuild();
    await waitFor(() => expect(stderr.text()).toContain("[inca] reload failed"));

    controller.abort();
    await running;

    expect(stderr.text()).toContain("[inca] reload failed: boom");
    expect(stderr.text()).toContain("Error: boom");
  });

  it("prints an appError distinctly from the host's ready notification", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: appErrorMockHost,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await waitFor(() => expect(stderr.text()).toContain("[inca] app error"));

    controller.abort();
    await running;

    expect(stderr.text()).toContain("[inca] app error: boom");
    expect(stderr.text()).toContain("Error: boom");
  });

  it("tears down both the host and the bundler watcher on abort", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: mockHost,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    controller.abort();
    await running;

    expect(bundler.closed).toBe(true);
  });

  it("defers a build that lands before ready, then reloads once instead of racing it", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: slowReadyMockHost,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    bundler.emitBuild();

    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));
    await waitFor(() => expect(stderr.text()).toContain("reload #1"));

    controller.abort();
    await running;

    expect(stderr.text()).not.toContain("reload failed");
    expect(stderr.text()).not.toContain("reload #2");
  });

  it("stops on its own once the host exits, without waiting for an abort", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: exitingMockHost,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: new AbortController().signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await running;

    expect(stdout.text()).toContain("[inca] host exited");
    expect(bundler.closed).toBe(true);
  });

  it.each([
    ["a non-zero code", "101", "code 101"],
    ["a signal", "SIGKILL", "signal SIGKILL"],
  ])(
    "rejects with ERR_INCA_HOST_CRASHED when the host exits by %s once ready",
    async (_, exit, said) => {
      vi.stubEnv("MOCK_HOST_EXIT", exit);
      onTestFinished(() => void vi.unstubAllEnvs());
      const bundler = makeFakeBundler();

      const running = dev({
        cwd: await scratch.makeApp({}),
        entry: "unused",
        bundler,
        hostBin: exitingMockHost,
        stdout: makeSink().stream,
        stderr: makeSink().stream,
        signal: new AbortController().signal,
      });

      await bundler.watching;
      bundler.emitBuild();
      await expect(running).rejects.toMatchObject({
        code: "ERR_INCA_HOST_CRASHED",
        message: expect.stringContaining(said),
      });
      expect(bundler.closed).toBe(true);
    },
  );

  it("resolves when the host is ended by SIGTERM once ready", async () => {
    vi.stubEnv("MOCK_HOST_EXIT", "SIGTERM");
    onTestFinished(() => void vi.unstubAllEnvs());
    const bundler = makeFakeBundler();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: exitingMockHost,
      stdout: makeSink().stream,
      stderr: makeSink().stream,
      signal: new AbortController().signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await expect(running).resolves.toBeUndefined();
  });

  it.each([
    ["speaks a protocol this package wasn't built for", protocolMismatchMockHost, "protocol 99"],
    ["reports an app that threw while loading", startupFailureMockHost, "[inca] app error: boom"],
  ])("rejects with ERR_INCA_HOST_EXITED_EARLY when the host %s", async (_, hostBin, printed) => {
    const bundler = makeFakeBundler();
    const stderr = makeSink();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin,
      stdout: makeSink().stream,
      stderr: stderr.stream,
      signal: new AbortController().signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await expect(running).rejects.toMatchObject({ code: "ERR_INCA_HOST_EXITED_EARLY" });

    expect(stderr.text()).toContain(printed);
    expect(bundler.closed).toBe(true);
  });

  it("refuses to start while another dev is running for the same app", async () => {
    const cwd = await scratch.makeApp({});
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd,
      entry: "unused",
      bundler,
      hostBin: mockHost,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    await expect(
      dev({
        cwd,
        entry: "unused",
        bundler: makeFakeBundler(),
        hostBin: mockHost,
        stdout: makeSink().stream,
        stderr: makeSink().stream,
        signal: new AbortController().signal,
      }),
    ).rejects.toThrow(
      expect.objectContaining({
        code: "ERR_INCA_DEV_RUNNING",
        message: expect.stringMatching(/already running for this app \(pid \d+\)/),
      }),
    );

    controller.abort();
    await running;
  });
});
