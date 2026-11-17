// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterAll, beforeAll, describe, expect, it, onTestFinished, vi } from "vitest";

import { scratchConfigApp } from "./scratchConfigApp.mts";
import {
  waitFor,
  mockHost,
  protocolMismatchMockHost,
  startupFailureMockHost,
  exitingMockHost,
  makeFakeBundler,
  makeSink,
  devWithoutHmr,
} from "./support/dev.mts";

describe("dev when the host ends or a second dev starts", () => {
  const scratch = scratchConfigApp("dev-host-exit");
  beforeAll(scratch.setUp);
  afterAll(scratch.tearDown);

  it("stops on its own once the host exits, without waiting for an abort", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();

    const running = devWithoutHmr({
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

      const running = devWithoutHmr({
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

    const running = devWithoutHmr({
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

    const running = devWithoutHmr({
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

    const running = devWithoutHmr({
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
      devWithoutHmr({
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
