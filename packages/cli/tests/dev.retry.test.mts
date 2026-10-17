// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, it, onTestFinished, vi } from "vitest";

import { dev } from "../src/dev.mts";
import { acquireDevLock } from "../src/devLock.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import { makeFakeBundler, makeSink, mockHost, waitFor } from "./support/dev.mts";
import { fsLag } from "./support/fsLag.mts";
import type { Bundler } from "../src/types.mts";

describe("a failure before the first build", () => {
  const scratch = scratchConfigApp("dev-retry");
  beforeAll(scratch.setUp);
  afterAll(scratch.tearDown);

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

    await waitFor(() =>
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

    await waitFor(() => expect(stderr.text()).toContain(testCase.said));
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
    await fsLag();
    const { bundler, stdout, stderr, watch } = startDev(cwd);
    await waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

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
    await waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

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

    await waitFor(() => expect(stderr.text()).toContain("ERR_INCA_OUT_DIR_INVALID"));
    expect(watch).not.toHaveBeenCalled();

    await writeFile(path.join(cwd, "inca.config.ts"), 'export default { outDir: "out" };\n');
    await bundler.watching;

    controller.abort();
    await running;
  });
});
