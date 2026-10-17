// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterAll, beforeAll, describe, expect, it, onTestFinished, vi } from "vitest";

import { dev } from "../src/dev.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import { waitFor, envMockHost, makeFakeBundler, makeSink } from "./support/dev.mts";

describe("the host's color environment", () => {
  const scratch = scratchConfigApp("dev-color");
  beforeAll(scratch.setUp);
  afterAll(scratch.tearDown);

  /** Starts the host against `stderr` and returns the env line it reported. */
  async function reportedEnv(stderr: ReturnType<typeof makeSink>): Promise<string> {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd: await scratch.makeApp({}),
      entry: "unused",
      bundler,
      hostBin: envMockHost,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await waitFor(() => expect(stderr.text()).toContain("env NO_COLOR="));
    controller.abort();
    await running;

    return /env NO_COLOR=\S+ FORCE_COLOR=\S+/.exec(stderr.text())?.[0] ?? "";
  }

  it.each([
    ["a terminal with neither variable set", true, undefined, undefined, "null", '"1"'],
    ["a terminal with an empty NO_COLOR", true, "", undefined, '""', '"1"'],
    ["a terminal with an empty FORCE_COLOR", true, undefined, "", "null", '"1"'],
    ["a pipe with neither variable set", false, undefined, undefined, "null", "null"],
    ["a terminal with NO_COLOR set", true, "1", undefined, '"1"', "null"],
    ["a terminal with FORCE_COLOR set", true, undefined, "0", "null", '"0"'],
    ["a pipe with FORCE_COLOR set", false, undefined, "2", "null", '"2"'],
  ])("passes the host %s", async (_, tty, noColor, forceColor, seenNoColor, seenForceColor) => {
    vi.stubEnv("NO_COLOR", noColor);
    vi.stubEnv("FORCE_COLOR", forceColor);
    onTestFinished(() => {
      vi.unstubAllEnvs();
    });

    const line = await reportedEnv(makeSink(tty));

    expect(line).toBe(`env NO_COLOR=${seenNoColor} FORCE_COLOR=${seenForceColor}`);
  });
});
