// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

vi.mock("../src/macosApp.mts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/macosApp.mts")>();
  return { ...actual, writeMacosApp: vi.fn<typeof actual.writeMacosApp>(actual.writeMacosApp) };
});

vi.mock("../src/config/loader.mts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/config/loader.mts")>();
  return {
    ...actual,
    runtimeConfigOf: vi.fn<typeof actual.runtimeConfigOf>(actual.runtimeConfigOf),
  };
});

vi.mock("../src/entry.mts", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/entry.mts")>();
  return { ...actual, resolveEntry: vi.fn<typeof actual.resolveEntry>(actual.resolveEntry) };
});

import { runtimeConfigOf } from "../src/config/loader.mts";
import { dev } from "../src/dev.mts";
import { resolveEntry } from "../src/entry.mts";
import { writeMacosApp } from "../src/macosApp.mts";
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

  it("assembles no bundle when the signal aborts while the runtime config is derived", async () => {
    pretendMacos();
    const cwd = await scratch.makeApp({ name: "deriving" });
    const controller = new AbortController();
    vi.mocked(writeMacosApp).mockClear();
    const derive = vi.mocked(runtimeConfigOf).getMockImplementation()!;
    vi.mocked(runtimeConfigOf).mockImplementationOnce((config, mode) => {
      controller.abort();
      return derive(config, mode);
    });

    await dev({
      cwd,
      entry: "unused",
      bundler: makeFakeBundler(),
      hostBin: mockHost,
      stdout: makeSink().stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    expect(writeMacosApp).not.toHaveBeenCalled();
  });

  it("prints no unbundled notice when the signal aborts while the bundle fails", async () => {
    pretendMacos();
    const cwd = await scratch.makeApp({ name: "failing" });
    const controller = new AbortController();
    const stdout = makeSink();
    vi.mocked(writeMacosApp).mockImplementationOnce(() => {
      controller.abort();
      return Promise.reject(new Error("no space"));
    });

    await dev({
      cwd,
      entry: "unused",
      bundler: makeFakeBundler(),
      hostBin: mockHost,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    expect(stdout.text()).not.toContain("running unbundled");
  });

  it("selects no entry when the signal aborts while the bundle is assembled", async () => {
    pretendMacos();
    const cwd = await scratch.makeApp({ name: "cancelling" });
    await mkdir(path.join(cwd, "src"), { recursive: true });
    await writeFile(path.join(cwd, "src/App.vue"), "<template><div/></template>\n");
    const controller = new AbortController();
    const assemble = vi.mocked(writeMacosApp).getMockImplementation()!;
    vi.mocked(resolveEntry).mockClear();
    vi.mocked(writeMacosApp).mockImplementationOnce((options) => {
      controller.abort();
      return assemble(options);
    });

    await dev({
      cwd,
      bundler: makeFakeBundler(),
      hostBin: mockHost,
      stdout: makeSink().stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    // The one call belongs to the config step.
    expect(resolveEntry).toHaveBeenCalledTimes(1);
  });
});
