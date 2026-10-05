// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { chmod, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { afterAll, beforeAll, describe, expect, it, onTestFinished, vi } from "vitest";

import { dev } from "../src/dev.mts";
import { acquireDevLock } from "../src/devLock.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import type {
  Bundler,
  BundlerOptions,
  BuildOutput,
  HmrOptions,
  Watcher,
} from "../src/adapter/types.mts";

const mockHost = path.join(import.meta.dirname, "fixtures/mock-host.mts");
const envMockHost = path.join(import.meta.dirname, "fixtures/mock-host-env.mts");
const reloadFailsMockHost = path.join(import.meta.dirname, "fixtures/mock-host-reload-fails.mts");
const appErrorMockHost = path.join(import.meta.dirname, "fixtures/mock-host-app-error.mts");
const protocolMismatchMockHost = path.join(
  import.meta.dirname,
  "dev-client/fixtures/mock-host-protocol-mismatch.mts",
);
const startupFailureMockHost = path.join(
  import.meta.dirname,
  "dev-client/fixtures/mock-host-startup-failure.mts",
);
const slowReadyMockHost = path.join(import.meta.dirname, "fixtures/mock-host-slow-ready.mts");
const exitingMockHost = path.join(import.meta.dirname, "fixtures/mock-host-exits.mts");
const hmrMockHost = path.join(import.meta.dirname, "fixtures/mock-host-hmr.mts");

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

/** Reports this process as `name` for one test. */
function pretendPlatform(name: NodeJS.Platform): void {
  const real = process.platform;
  Object.defineProperty(process, "platform", { value: name, configurable: true });
  onTestFinished(() => {
    Object.defineProperty(process, "platform", { value: real, configurable: true });
  });
}

/** A host binary in `dir` that exists but can't be spawned. */
async function makeUnspawnableHost(dir: string): Promise<string> {
  const hostBin = path.join(dir, "inca-host");
  await writeFile(hostBin, "#!/bin/sh\n");
  await chmod(hostBin, 0o644);
  return hostBin;
}

function makeSink(tty = false): { stream: NodeJS.WritableStream; text: () => string } {
  let data = "";
  const stream = new Writable({
    write(chunk: Buffer, _encoding, callback) {
      data += chunk.toString();
      callback();
    },
  });
  if (tty) Object.assign(stream, { isTTY: true });
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

  describe("the host's colour environment", () => {
    /** Starts the host against `stderr` and returns the env line it reported. */
    async function reportedEnv(stderr: ReturnType<typeof makeSink>): Promise<string> {
      const bundler = makeFakeBundler();
      const stdout = makeSink();
      const controller = new AbortController();

      const running = dev({
        entry: "unused",
        bundler,
        hostBin: envMockHost,
        stdout: stdout.stream,
        stderr: stderr.stream,
        signal: controller.signal,
      });

      await bundler.watching;
      bundler.emitBuild();
      await vi.waitFor(() => expect(stderr.text()).toContain("env NO_COLOR="));
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

  it("rejects before watching and releases the lock when the host binary is missing", async () => {
    const bundler = makeFakeBundler();
    const watch = vi.spyOn(bundler, "watch");
    const options = {
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
        entry: "unused",
        bundler,
        stdout: makeSink().stream,
        stderr: makeSink().stream,
        signal: new AbortController().signal,
      }),
    ).rejects.toMatchObject({ code: "ERR_INCA_HOST_BIN_UNRESOLVED" });
    expect(watch).not.toHaveBeenCalled();
  });

  it("prints a build failure without ever starting the host", async () => {
    const hostBin = await makeUnspawnableHost(await scratch.makeApp({}));
    const bundler = makeFakeBundler();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
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
    await vi.waitFor(() => expect(stderr.text()).toContain("[inca] build failed"));

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

  describe("a failure before the first build", () => {
    /** Starts `dev()` on `cwd` with sinks the test reads back. */
    function startDev(cwd: string, overrides: Partial<Parameters<typeof dev>[0]> = {}) {
      const bundler = makeFakeBundler();
      const watch = vi.spyOn(bundler, "watch");
      const stdout = makeSink();
      const stderr = makeSink();
      const controller = new AbortController();
      const running = dev({
        cwd,
        bundler,
        hostBin: mockHost,
        stdout: stdout.stream,
        stderr: stderr.stream,
        signal: controller.signal,
        ...overrides,
      });
      onTestFinished(async () => {
        controller.abort();
        await running.catch(() => {});
      });
      return { bundler, watch, stdout, stderr, controller, running };
    }

    /** How many times `needle` appears in `text`. */
    function count(text: string, needle: string): number {
      return text.split(needle).length - 1;
    }

    /** Claims and releases the lock on `cwd`, which fails while another `dev()` holds it. */
    async function expectLockFree(cwd: string): Promise<void> {
      const release = await acquireDevLock(cwd);
      await release();
    }

    /** An app with a `src/main.mts`. */
    async function makeAppWithEntry(pkg: object = { name: "retry" }): Promise<string> {
      const cwd = await scratch.makeApp(pkg);
      await mkdir(path.join(cwd, "src"), { recursive: true });
      await writeFile(path.join(cwd, "src/main.mts"), "");
      return cwd;
    }

    it("waits for src/main.mts when there is no entry, then starts the watcher", async () => {
      const cwd = await scratch.makeApp({ name: "no-entry" });
      const first = startDev(cwd);

      await vi.waitFor(() =>
        expect(first.stderr.text()).toContain("[inca] build failed (ERR_INCA_ENTRY_NOT_FOUND)"),
      );
      expect(first.stdout.text()).toContain("[inca] waiting for a change to retry");
      expect(first.watch).not.toHaveBeenCalled();

      await mkdir(path.join(cwd, "src"));
      await writeFile(path.join(cwd, "src/main.mts"), "");
      await first.bundler.watching;
      expect(first.stdout.text()).toContain("[inca] retrying");

      first.controller.abort();
      await first.running;

      // The lock is free again, so a second dev on the same app starts.
      const second = startDev(cwd);
      await second.bundler.watching;
      second.controller.abort();
      await second.running;
    });

    it.each([
      {
        name: "a config that throws",
        pkg: { name: "cfg" },
        break: (cwd: string) =>
          writeFile(path.join(cwd, "inca.config.ts"), 'throw new Error("config exploded");\n'),
        said: "config exploded",
        fix: (cwd: string) => writeFile(path.join(cwd, "inca.config.ts"), "export default {};\n"),
      },
      {
        name: "a malformed package.json",
        pkg: { name: "pkg" },
        break: (cwd: string) => writeFile(path.join(cwd, "package.json"), "{ not json"),
        said: "[inca] build failed",
        fix: (cwd: string) =>
          writeFile(path.join(cwd, "package.json"), JSON.stringify({ type: "module", name: "ok" })),
      },
      {
        name: "an outDir that holds the app",
        pkg: { name: "out" },
        break: (cwd: string) =>
          writeFile(path.join(cwd, "inca.config.ts"), 'export default { outDir: "." };\n'),
        said: "ERR_INCA_OUT_DIR_INVALID",
        fix: (cwd: string) => writeFile(path.join(cwd, "inca.config.ts"), "export default {};\n"),
      },
      {
        name: "a missing icon file, fixed by creating it",
        pkg: { name: "icon" },
        break: (cwd: string) =>
          writeFile(path.join(cwd, "inca.config.ts"), 'export default { icon: "./app.icns" };\n'),
        said: "ERR_INCA_ICON_NOT_FOUND",
        fix: (cwd: string) => writeFile(path.join(cwd, "app.icns"), ""),
      },
      {
        name: "a missing icon file, fixed by the config",
        pkg: { name: "icon" },
        break: (cwd: string) =>
          writeFile(path.join(cwd, "inca.config.ts"), 'export default { icon: "./app.icns" };\n'),
        said: "ERR_INCA_ICON_NOT_FOUND",
        fix: (cwd: string) => writeFile(path.join(cwd, "inca.config.ts"), "export default {};\n"),
      },
      {
        name: "a product name that cannot name a directory",
        pkg: { name: "name" },
        break: (cwd: string) =>
          writeFile(path.join(cwd, "inca.config.ts"), 'export default { productName: "a/b" };\n'),
        said: "ERR_INCA_PRODUCT_NAME_INVALID",
        fix: (cwd: string) => writeFile(path.join(cwd, "inca.config.ts"), "export default {};\n"),
      },
    ])("reports $name and starts once it is fixed", async (testCase) => {
      const cwd = await makeAppWithEntry(testCase.pkg);
      await testCase.break(cwd);
      const { bundler, watch, stdout, stderr, controller, running } = startDev(cwd);

      await vi.waitFor(() => expect(stderr.text()).toContain(testCase.said));
      expect(stderr.text()).toContain("[inca] build failed");
      expect(stdout.text()).toContain("[inca] waiting for a change to retry");
      expect(watch).not.toHaveBeenCalled();

      await testCase.fix(cwd);
      await bundler.watching;
      expect(count(stderr.text(), "[inca] build failed")).toBe(1);

      controller.abort();
      await running;
    });

    it("does not print the fault again for an unrelated edit", async () => {
      const cwd = await scratch.makeApp({ name: "unrelated" });
      await mkdir(path.join(cwd, "src"));
      const { bundler, stdout, stderr, watch } = startDev(cwd);
      await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

      await writeFile(path.join(cwd, "src/util.mts"), "");
      await writeFile(path.join(cwd, "README.md"), "");
      await new Promise((resolve) => setTimeout(resolve, 400));

      expect(count(stderr.text(), "[inca] build failed")).toBe(1);
      expect(watch).not.toHaveBeenCalled();

      // The watcher is alive, so an edit that counts is seen.
      await writeFile(path.join(cwd, "src/main.mts"), "");
      await bundler.watching;
    });

    it("resolves on abort while waiting, frees the lock and prints nothing more", async () => {
      const cwd = await scratch.makeApp({ name: "abort-wait" });
      const { stdout, stderr, controller, running, watch } = startDev(cwd);
      await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

      controller.abort();
      await expect(running).resolves.toBeUndefined();
      await mkdir(path.join(cwd, "src"));
      await writeFile(path.join(cwd, "src/main.mts"), "");
      await new Promise((resolve) => setTimeout(resolve, 400));

      expect(count(stderr.text(), "[inca] build failed")).toBe(1);
      expect(watch).not.toHaveBeenCalled();
      await expect(acquireDevLock(cwd)).resolves.toBeTypeOf("function");
    });

    it("rejects with the host error and prints no build failure when the host is missing", async () => {
      const cwd = await makeAppWithEntry();
      await writeFile(path.join(cwd, "inca.config.ts"), 'throw new Error("config exploded");\n');
      const { stderr, stdout, watch, running } = startDev(cwd, {
        hostBin: "/nonexistent/inca-host",
      });

      await expect(running).rejects.toMatchObject({ code: "ERR_INCA_HOST_BIN_NOT_FOUND" });

      expect(stderr.text()).not.toContain("build failed");
      expect(stdout.text()).toBe("");
      expect(watch).not.toHaveBeenCalled();
      await expectLockFree(cwd);
    });

    it("reports a held lock before it reads a broken config", async () => {
      const cwd = await makeAppWithEntry();
      await writeFile(path.join(cwd, "inca.config.ts"), 'throw new Error("config exploded");\n');
      const release = await acquireDevLock(cwd);
      onTestFinished(release);
      const { stderr, running } = startDev(cwd);

      await expect(running).rejects.toMatchObject({ code: "ERR_INCA_DEV_RUNNING" });

      expect(stderr.text()).toBe("");
    });

    it("rejects with ERR_INCA_HMR_UNSUPPORTED at once, even with a broken config", async () => {
      const cwd = await makeAppWithEntry();
      await writeFile(path.join(cwd, "inca.config.ts"), 'throw new Error("config exploded");\n');
      const bundler: Bundler = {
        watch: () => Promise.reject(new Error("not used")),
        build: () => Promise.reject(new Error("not used")),
      };
      const { stdout, stderr, running } = startDev(cwd, { bundler, experimentalHmr: true });

      await expect(running).rejects.toMatchObject({ code: "ERR_INCA_HMR_UNSUPPORTED" });

      expect(stderr.text()).toBe("");
      expect(stdout.text()).toBe("");
      await expectLockFree(cwd);
    });

    it("reports an entry override inside outDir and starts once outDir moves", async () => {
      const cwd = await scratch.makeApp({ name: "entry-in-out-dir" });
      const { bundler, watch, stderr, controller, running } = startDev(cwd, {
        entry: path.join(cwd, "dist", "main.mts"),
      });

      await vi.waitFor(() => expect(stderr.text()).toContain("ERR_INCA_OUT_DIR_INVALID"));
      expect(watch).not.toHaveBeenCalled();

      await writeFile(path.join(cwd, "inca.config.ts"), 'export default { outDir: "out" };\n');
      await bundler.watching;

      controller.abort();
      await running;
    });
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

interface FakeHmrBundler extends Bundler {
  /** Simulates the running app pushing a `"vite"` payload out to the dev server. */
  emitVite(payload: unknown): void;
  /** Simulates the environment's `HotChannel` seeing a `full-reload` payload. */
  emitFullReload(): void;
  /** Simulates a failed update the bundler reports. */
  emitError(error: { message: string; stack: string | null }): void;
  /** Every payload `hmr()`'s channel was asked to `dispatch` back into the app. */
  dispatched: unknown[];
}

function makeFakeHmrBundler(entryFile: string): FakeHmrBundler {
  let options: HmrOptions | undefined;
  const dispatched: unknown[] = [];

  const fake: FakeHmrBundler = {
    watch: () => Promise.reject(new Error("not used by HMR mode")),
    build: () => Promise.reject(new Error("not used by HMR mode")),
    hmr(opts) {
      options = opts;
      return Promise.resolve({
        entryFile,
        dispatch: (payload) => dispatched.push(payload),
        close: () => Promise.resolve(),
      });
    },
    emitVite(payload) {
      options?.notify(payload);
    },
    emitFullReload() {
      options?.reload();
    },
    emitError(error) {
      options?.onError(error);
    },
    dispatched,
  };
  return fake;
}

describe("dev with --experimental-hmr", () => {
  const scratch = scratchConfigApp("dev-hmr");
  beforeAll(scratch.setUp);
  afterAll(scratch.tearDown);

  it("roundtrips a vite payload the app sends back through the dev channel", async () => {
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const stdout = makeSink();
    const controller = new AbortController();

    const running = dev({
      entry: "unused",
      bundler,
      hostBin: hmrMockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: makeSink().stream,
      signal: controller.signal,
    });

    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    bundler.emitVite({ type: "custom", event: "greet", data: { hi: 1 } });
    await vi.waitFor(() =>
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
      entry: "unused",
      bundler,
      hostBin: slowReadyMockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    bundler.emitFullReload();
    await vi.waitFor(() => expect(stderr.text()).toContain("reload #1"));

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
      entry: "unused",
      bundler,
      hostBin: mockHost,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    bundler.emitError({ message: "bad template", stack: "at somewhere" });
    await vi.waitFor(() => expect(stderr.text()).toContain("[inca] build failed"));

    controller.abort();
    await running;

    expect(stderr.text()).toContain("[inca] build failed: bad template");
    expect(stderr.text()).toContain("at somewhere");
  });

  it("stops instead of hanging when the host fails to start", async () => {
    const hostBin = await makeUnspawnableHost(await scratch.makeApp({}));
    const bundler = makeFakeHmrBundler("/unused/entry.js");
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();

    const running = dev({
      entry: "unused",
      bundler,
      hostBin,
      experimentalHmr: true,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
    });

    // HMR mode has no rebuild loop to retry a failed start on — dev()
    // itself must resolve rather than wait forever for a signal that
    // will never come.
    await running;
    expect(stderr.text()).toContain("failed to start inca-host");

    expect(stdout.text()).not.toContain("[inca] ready");
  });

  it("throws a clear error when the bundler has no HMR support", async () => {
    const bundler: Bundler = {
      watch: () => Promise.reject(new Error("not used")),
      build: () => Promise.reject(new Error("not used")),
    };

    await expect(
      dev({
        entry: "unused",
        bundler,
        hostBin: mockHost,
        experimentalHmr: true,
        stdout: makeSink().stream,
        stderr: makeSink().stream,
        signal: new AbortController().signal,
      }),
    ).rejects.toThrow(expect.objectContaining({ code: "ERR_INCA_HMR_UNSUPPORTED" }));
  });
});

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
    await vi.waitFor(() => expect(stdout.text()).toContain("[inca] ready"));

    controller.abort();
    await running;

    expect(stdout.text()).toContain("running unnamed");
    const scratchFiles = await readdir(path.join(cwd, "node_modules/.inca"));
    expect(scratchFiles.filter((name) => name.endsWith(".app"))).toEqual([]);
  });
});
