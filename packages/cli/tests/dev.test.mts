// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readFile, readdir, stat } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { afterAll, beforeAll, describe, expect, it, onTestFinished, vi } from "vitest";

import { dev } from "../src/dev.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import type { Bundler, BundlerOptions, BuildOutput, Watcher } from "../src/adapter/types.mts";

const mockHost = path.join(import.meta.dirname, "fixtures/mock-host.mts");
const reloadFailsMockHost = path.join(import.meta.dirname, "fixtures/mock-host-reload-fails.mts");
const appErrorMockHost = path.join(import.meta.dirname, "fixtures/mock-host-app-error.mts");
const slowReadyMockHost = path.join(import.meta.dirname, "fixtures/mock-host-slow-ready.mts");
const exitingMockHost = path.join(import.meta.dirname, "fixtures/mock-host-exits.mts");

// A nonexistent directory: pruneStaleFiles() treats a missing outDir as
// nothing to prune, so these tests need no real files on disk.
const FAKE_OUTPUT: BuildOutput = {
  outDir: "/unused",
  entryFile: "/unused/bundle.js",
  files: ["bundle.js"],
};

interface FakeBundler extends Bundler {
  /** Simulates a successful (re)build. */
  emitBuild(output?: BuildOutput): void;
  /** Simulates a build failure. */
  emitError(error: { message: string; stack: string | null }): void;
  closed: boolean;
  /**
   * Resolves once `watch()` is called — `dev()` now resolves its config
   * (an async step) before reaching `watch()`, so a test firing an event
   * right after calling `dev()` has to wait for this first, or the event
   * lands before any handler is registered to see it.
   */
  watching: Promise<void>;
}

function makeFakeBundler(): FakeBundler {
  let handlers: Pick<BundlerOptions, "onBuild" | "onError"> | undefined;
  let markWatching: () => void;
  const watching = new Promise<void>((resolve) => {
    markWatching = resolve;
  });

  const fake: FakeBundler = {
    closed: false,
    watching,
    watch(options): Promise<Watcher> {
      handlers = options;
      markWatching();
      return Promise.resolve({
        close: () => {
          fake.closed = true;
          return Promise.resolve();
        },
      });
    },
    // Never exercised here — dev() only ever calls watch().
    build: () => Promise.reject(new Error("not used by dev()")),
    emitBuild(output = FAKE_OUTPUT) {
      handlers?.onBuild(output);
    },
    emitError(error) {
      handlers?.onError(error);
    },
  };
  return fake;
}

function makeSink(): { stream: NodeJS.WritableStream; text: () => string } {
  let data = "";
  const stream = new Writable({
    write(chunk: Buffer, _encoding, callback) {
      data += chunk.toString();
      callback();
    },
  });
  return { stream, text: () => data };
}

describe("dev", () => {
  const scratch = scratchConfigApp("dev");
  beforeAll(scratch.setUp);
  afterAll(scratch.tearDown);

  it("starts the host and prints ready once the first build lands", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const controller = new AbortController();

    const running = dev({
      entry: "unused",
      bundler,
      hostBin: mockHost,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

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
      entry: "unused",
      bundler,
      hostBin: mockHost,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));
    bundler.emitBuild();

    controller.abort();
    await running;

    expect(stderr.text()).not.toContain("reload failed");
  });

  it("names the file that triggered a reload and how long it took", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const controller = new AbortController();

    const running = dev({
      entry: "unused",
      bundler,
      hostBin: mockHost,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));
    expect(stdout.text()).not.toContain("reload");

    const vuePath = path.join(process.cwd(), "src/App.vue");
    bundler.emitBuild({ ...FAKE_OUTPUT, changed: { file: vuePath, at: Date.now() } });
    await vi.waitFor(() => expect(stdout.text()).toContain("reload src/App.vue"));

    controller.abort();
    await running;

    expect(stdout.text()).toMatch(/\[inca\] reload src\/App\.vue \(\d+ms\)/);
  });

  it("prints a build failure without ever starting the host", async () => {
    const bundler = makeFakeBundler();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      entry: "unused",
      bundler,
      // Deliberately unspawnable: a build failure must never reach the
      // point of starting a host at all.
      hostBin: "/nonexistent/inca-host",
      stdout: makeSink().stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitError({ message: "syntax error", stack: "at somewhere" });
    await vi.waitFor(() => expect(stderr.text()).toContain("[inca] build failed"));

    controller.abort();
    await running;

    expect(stderr.text()).toContain("[inca] build failed: syntax error");
    expect(stderr.text()).toContain("at somewhere");
  });

  it("prints a failed reload (-32000) without throwing", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      entry: "unused",
      bundler,
      hostBin: reloadFailsMockHost,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));
    bundler.emitBuild();
    await vi.waitFor(() => expect(stderr.text()).toContain("[inca] reload failed"));

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
      entry: "unused",
      bundler,
      hostBin: appErrorMockHost,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await vi.waitFor(() => expect(stderr.text()).toContain("[inca] app error"));

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
      entry: "unused",
      bundler,
      hostBin: mockHost,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

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

    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));
    await vi.waitFor(() => expect(stderr.text()).toContain("reload #1"));

    controller.abort();
    await running;

    expect(stderr.text()).not.toContain("reload failed");
    expect(stderr.text()).not.toContain("reload #2");
  });

  it("stops on its own once the host exits, without waiting for an abort", async () => {
    const bundler = makeFakeBundler();
    const stdout = makeSink();

    const running = dev({
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

  it("releases its lock when it fails before the watcher starts", async () => {
    const cwd = await scratch.makeApp({ name: "no-entry" });
    const failing = {
      cwd,
      bundler: makeFakeBundler(),
      hostBin: mockHost,
      stdout: makeSink().stream,
      stderr: makeSink().stream,
      signal: new AbortController().signal,
    };

    // No `entry` and no `src/` — resolveEntry throws before the lock's own
    // release would run.
    await expect(dev(failing)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_ENTRY_NOT_FOUND" }),
    );

    await expect(dev(failing)).rejects.toThrow(
      expect.objectContaining({ code: "ERR_INCA_ENTRY_NOT_FOUND" }),
    );
  });

  it("refuses to start while another dev is running for the same app", async () => {
    const cwd = await scratch.makeApp({ name: "locked" });
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
    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

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

describe("dev on macOS", () => {
  const scratch = scratchConfigApp("dev-macos");
  beforeAll(scratch.setUp);
  afterAll(scratch.tearDown);

  /** Reports this process as macOS for one test, so the bundling path runs off one. */
  function pretendMacos(): void {
    const real = process.platform;
    Object.defineProperty(process, "platform", { value: "darwin", configurable: true });
    onTestFinished(() => {
      Object.defineProperty(process, "platform", { value: real, configurable: true });
    });
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

  it("keeps running when no host binary resolves", async () => {
    pretendMacos();
    const cwd = await scratch.makeApp({ name: "hostless" });
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      cwd,
      entry: "unused",
      bundler,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await bundler.watching;
    bundler.emitBuild();
    await vi.waitFor(() => expect(stderr.text()).toContain("failed to start inca-host"));

    controller.abort();
    await running;

    expect(stdout.text()).toContain("running unbundled");
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
    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    controller.abort();
    await running;

    expect(stdout.text()).toContain("running unnamed");
    const scratchFiles = await readdir(path.join(cwd, "node_modules/.inca"));
    expect(scratchFiles.filter((name) => name.endsWith(".app"))).toEqual([]);
  });
});
