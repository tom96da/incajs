// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readFile, readdir, stat } from "node:fs/promises";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { dev } from "../src/dev.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import { waitFor, mockHost, makeFakeBundler, pretendPlatform, makeSink } from "./support/dev.mts";

describe("dev on macOS", () => {
  const scratch = scratchConfigApp("dev-macos");
  beforeAll(scratch.setUp);
  afterAll(scratch.tearDown);

  /** Reports this process as macOS for one test, so the bundling path runs off one. */
  function pretendMacos(): void {
    pretendPlatform("darwin");
  }

  it("assembles a bundle named after the app before starting the host", async () => {
    pretendMacos();
    const cwd = await scratch.makeApp({ name: "bundled", version: "4.5.6" });
    const bundler = makeFakeBundler();
    const controller = new AbortController();

    const running = dev({
      cwd,
      entry: "unused",
      bundler,
      hostBin: mockHost,
      stdout: makeSink().stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    // The bundle is assembled before the watcher starts, so nothing has to
    // build or run for it to be on disk.
    await bundler.watching;

    const appPath = path.join(cwd, "node_modules/.inca/bundled.app");
    expect((await stat(path.join(appPath, "Contents/MacOS/bundled"))).ino).toBe(
      (await stat(mockHost)).ino,
    );
    const plist = await readFile(path.join(appPath, "Contents/Info.plist"), "utf8");
    expect(plist).toContain("<string>bundled</string>");
    expect(plist).toContain("<string>4.5.6</string>");

    controller.abort();
    await running;
  });

  it("rejects before bundling when no host binary is found", async () => {
    pretendMacos();
    const cwd = await scratch.makeApp({ name: "hostless" });
    const stdout = makeSink();

    await expect(
      dev({
        cwd,
        entry: "unused",
        bundler: makeFakeBundler(),
        hostBin: "/nonexistent/inca-host",
        stdout: stdout.stream,
        stderr: makeSink().stream,
        signal: new AbortController().signal,
      }),
    ).rejects.toMatchObject({ code: "ERR_INCA_HOST_BIN_NOT_FOUND" });

    expect(stdout.text()).not.toContain("running unbundled");
  });

  it("runs unnamed and says so when the app names itself nowhere", async () => {
    pretendMacos();
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

    controller.abort();
    await running;

    expect(stdout.text()).toContain("running unnamed");
    const scratchFiles = await readdir(path.join(cwd, "node_modules/.inca"));
    expect(scratchFiles.filter((name) => name.endsWith(".app"))).toEqual([]);
  });
});
