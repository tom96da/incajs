// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { rmSync } from "node:fs";
import { mkdir, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";

import { afterAll, beforeAll, describe, expect, it, onTestFinished, vi } from "vitest";

import { dev } from "../src/dev.mts";
import { scratchConfigApp } from "./scratchConfigApp.mts";
import {
  waitFor,
  logMockHost,
  useHostLog,
  FAKE_OUTPUT,
  makeFakeBundler,
  makeSink,
} from "./support/dev.mts";
import { fsLag } from "./support/fsLag.mts";
import { watches } from "./support/watches.mts";
import type { DevOptions } from "../src/dev.mts";
import type { BuildOutput } from "../src/types.mts";

vi.mock("../src/watchForEdit.mts", async (importOriginal) => {
  const { recordWatches } = await import("./support/watches.mts");
  return recordWatches(await importOriginal());
});

describe("dev entry selection", () => {
  const scratch = scratchConfigApp("dev-entry");
  beforeAll(scratch.setUp);
  afterAll(scratch.tearDown);

  const APP_VUE = "<template><div/></template>\n";
  const BUILD_A: BuildOutput = { ...FAKE_OUTPUT, entryFile: "/unused/a.js" };
  const BUILD_B: BuildOutput = { ...FAKE_OUTPUT, entryFile: "/unused/b.js" };

  /** An app whose `src/` holds the named files. */
  async function makeSourceApp(...names: string[]): Promise<string> {
    const cwd = await scratch.makeApp({});
    await mkdir(path.join(cwd, "src"), { recursive: true });
    for (const name of names) {
      await writeFile(path.join(cwd, "src", name), name === "App.vue" ? APP_VUE : "");
    }
    return cwd;
  }

  /** Runs `dev()` against `cwd` with the fake bundler and the logging host. */
  async function startDev(cwd: string, options: Partial<DevOptions> = {}) {
    const bundler = makeFakeBundler();
    const stdout = makeSink();
    const stderr = makeSink();
    const controller = new AbortController();
    const hostLog = await useHostLog(cwd);
    onTestFinished(() => controller.abort());
    const running = dev({
      cwd,
      bundler,
      hostBin: logMockHost,
      stdout: stdout.stream,
      stderr: stderr.stream,
      signal: controller.signal,
      ...options,
    });
    return {
      bundler,
      stdout,
      stderr,
      hostLog,
      running,
      abort: () => controller.abort(),
      readyCount: () => stdout.text().match(/\[inca\] ready/g)?.length ?? 0,
    };
  }

  /** Starts `dev()`, lets the first build land and waits for the host to be ready. */
  async function startReady(cwd: string, options: Partial<DevOptions> = {}) {
    const run = await startDev(cwd, options);
    await run.bundler.watching;
    run.bundler.emitBuild(BUILD_A);
    await waitFor(() => expect(run.readyCount()).toBe(1));
    return run;
  }

  /** Resolves once the entry monitor has taken one more edit signal and re-armed. */
  async function waitForSignal(): Promise<void> {
    const before = watches.list.length;
    await waitFor(() => expect(watches.list.length).toBeGreaterThan(before));
  }

  /** Waits until the mock hosts have logged exactly `expected`. */
  const hostEvents = (run: { hostLog: { lines: () => Promise<string[]> } }, expected: string[]) =>
    waitFor(async () => expect(await run.hostLog.lines()).toEqual(expected));

  /**
   * Leaves a reload in flight on the first host, which answers it once `release()` is called.
   */
  async function startSlowReload(cwd: string) {
    vi.stubEnv("MOCK_HOST_RELOAD_GATE", "1");
    const run = await startReady(cwd);
    run.bundler.emitBuild({ ...BUILD_A, changed: { file: path.join(cwd, "src/main.js"), at: 0 } });
    await hostEvents(run, ["start /unused/a.js", "reload"]);
    return { ...run, release: () => writeFile(`${path.join(cwd, "host.log")}.release`, "") };
  }

  it("reloads the host when a higher-priority entry appears", async () => {
    const cwd = await makeSourceApp("main.js");
    const run = await startReady(cwd);

    await writeFile(path.join(cwd, "src/main.mts"), "");
    await waitFor(() => expect(run.bundler.watches).toHaveLength(2));

    const [old, next] = run.bundler.watches;
    expect(old!.closed).toBe(true);
    expect(next!.openBefore).toBe(0);
    expect(next!.options).toMatchObject({
      entry: path.join(cwd, "src/main.mts"),
      root: path.join(cwd, "src"),
    });
    // The host reloads only once the new bundle exists.
    expect(await run.hostLog.lines()).toEqual(["start /unused/a.js"]);

    run.bundler.emitBuild(BUILD_B);
    await hostEvents(run, ["start /unused/a.js", "reload"]);
    expect(run.readyCount()).toBe(1);

    run.abort();
    await run.running;
  });

  it("reloads the host when the selected entry is deleted", async () => {
    const cwd = await makeSourceApp("main.mts", "App.vue");
    const run = await startReady(cwd);

    await rm(path.join(cwd, "src/main.mts"));
    await waitFor(() => expect(run.bundler.watches).toHaveLength(2));

    const synthesized = path.join(cwd, "node_modules/.inca/entry.mts");
    expect(run.bundler.watches[1]!.options).toMatchObject({
      entry: synthesized,
      root: path.join(cwd, "src"),
    });

    run.bundler.emitBuild(BUILD_B);
    await hostEvents(run, ["start /unused/a.js", "reload"]);

    run.abort();
    await run.running;
  });

  it("keeps the host when the selected entry is edited", async () => {
    const cwd = await makeSourceApp("main.mts");
    const run = await startReady(cwd);

    const signalled = waitForSignal();
    await writeFile(path.join(cwd, "src/main.mts"), "// edited\n");
    await signalled;
    // The build runs in the queue behind the selection, so its reload shows the host survived.
    run.bundler.emitBuild({ ...BUILD_A, changed: { file: path.join(cwd, "src/main.mts"), at: 0 } });
    await hostEvents(run, ["start /unused/a.js", "reload"]);

    expect(run.bundler.watches).toHaveLength(1);

    run.abort();
    await run.running;
  });

  it("does not rewrite the synthesized entry when App.vue is edited", async () => {
    const cwd = await makeSourceApp("App.vue");
    const run = await startReady(cwd);
    const synthesized = path.join(cwd, "node_modules/.inca/entry.mts");
    const written = (await stat(synthesized)).mtimeMs;

    // Two edits: the second signal arrives only after the first selection has finished.
    for (let edit = 0; edit < 2; edit++) {
      const signalled = waitForSignal();
      await writeFile(path.join(cwd, "src/App.vue"), `${APP_VUE}<!-- ${edit} -->\n`);
      await signalled;
      await fsLag();
    }

    expect((await stat(synthesized)).mtimeMs).toBe(written);
    expect(run.bundler.watches).toHaveLength(1);

    run.abort();
    await run.running;
  });

  it("keeps an explicit entry when main files change", async () => {
    const cwd = await makeSourceApp("App.vue");
    const explicit = path.join(cwd, "src/App.vue");
    const run = await startReady(cwd, { entry: explicit });

    await writeFile(path.join(cwd, "src/main.mts"), "");
    run.bundler.emitBuild({ ...BUILD_A, changed: { file: explicit, at: 0 } });
    await hostEvents(run, ["start /unused/a.js", "reload"]);

    expect(run.bundler.watches).toHaveLength(1);
    expect(run.bundler.watches[0]!.options.entry).toBe(explicit);
    // Only the retry watch of the first attempt was ever opened, and it is closed again.
    expect(watches.list).toEqual([{ closed: true }]);

    run.abort();
    await run.running;
  });

  it("keeps a config entry when main files change", async () => {
    const cwd = await makeSourceApp("App.vue");
    await writeFile(path.join(cwd, "inca.config.ts"), 'export default { entry: "src/App.vue" };\n');
    const run = await startReady(cwd);

    await writeFile(path.join(cwd, "src/main.mts"), "");
    run.bundler.emitBuild({ ...BUILD_A, changed: { file: path.join(cwd, "src/App.vue"), at: 0 } });
    await hostEvents(run, ["start /unused/a.js", "reload"]);

    expect(run.bundler.watches).toHaveLength(1);
    expect(run.bundler.watches[0]!.options.entry).toBe(path.join(cwd, "src/App.vue"));
    expect(watches.list).toEqual([{ closed: true }]);

    run.abort();
    await run.running;
  });

  it("prints a missing entry once while it stays missing", async () => {
    const cwd = await makeSourceApp("App.vue");
    const run = await startReady(cwd);
    const failures = () => run.stderr.text().match(/build failed/g)?.length ?? 0;

    await rm(path.join(cwd, "src/App.vue"));
    await waitFor(() => expect(run.stderr.text()).toContain("no app entry found"));
    // The old watcher is closed and the host keeps running.
    expect(run.bundler.watches[0]!.closed).toBe(true);
    expect(await run.hostLog.lines()).toEqual(["start /unused/a.js"]);

    // The monitor re-arms only after the previous selection finished, so the third signal
    // arrives after two selections ran while the entry was still missing.
    for (const edit of [1, 2, 3]) {
      await fsLag();
      const signalled = waitForSignal();
      await writeFile(path.join(cwd, "package.json"), JSON.stringify({ type: "module", edit }));
      await signalled;
    }

    expect(failures()).toBe(1);

    run.abort();
    await run.running;
  });

  it("reloads the host when the missing entry returns", async () => {
    const cwd = await makeSourceApp("App.vue");
    const run = await startReady(cwd);

    await rm(path.join(cwd, "src/App.vue"));
    await waitFor(() => expect(run.stderr.text()).toContain("no app entry found"));
    await fsLag();

    await writeFile(path.join(cwd, "src/App.vue"), APP_VUE);
    await waitFor(() => expect(run.bundler.watches).toHaveLength(2));
    expect(run.bundler.watches[1]!.options.entry).toBe(
      path.join(cwd, "node_modules/.inca/entry.mts"),
    );
    run.bundler.emitBuild(BUILD_B);
    await hostEvents(run, ["start /unused/a.js", "reload"]);

    run.abort();
    await run.running;
  });

  it("stops when the host exits while the entry is missing", async () => {
    const cwd = await makeSourceApp("App.vue");
    const run = await startReady(cwd);

    await rm(path.join(cwd, "src/App.vue"));
    await waitFor(() => expect(run.stderr.text()).toContain("no app entry found"));
    process.kill(await run.hostLog.pid(), "SIGTERM");
    await run.running;

    expect(run.stdout.text()).toContain("[inca] host exited");
    expect(watches.allClosed()).toBe(true);
  });

  it("waits for a running reload before it switches the entry", async () => {
    const cwd = await makeSourceApp("main.js");
    const run = await startSlowReload(cwd);

    const signalled = waitForSignal();
    await writeFile(path.join(cwd, "src/main.mts"), "");
    await signalled;
    await new Promise((resolve) => setTimeout(resolve, 300));
    expect(run.bundler.watches).toHaveLength(1);
    expect(run.bundler.watches[0]!.closed).toBe(false);

    await run.release();
    await hostEvents(run, ["start /unused/a.js", "reload", "reloaded"]);
    await waitFor(() => expect(run.bundler.watches).toHaveLength(2));
    expect(run.bundler.watches[0]!.closed).toBe(true);

    run.abort();
    await run.running;
  });

  it("skips a build that waits in the queue when shutdown starts", async () => {
    const cwd = await makeSourceApp("main.js");
    const run = await startSlowReload(cwd);

    run.bundler.emitBuild(BUILD_A);
    run.abort();
    await run.release();
    await run.running;

    expect((await run.hostLog.lines()).filter((line) => line === "reload")).toHaveLength(1);
  });

  it("does not switch the entry once shutdown has started", async () => {
    const cwd = await makeSourceApp("main.js");
    const run = await startSlowReload(cwd);

    const signalled = waitForSignal();
    await writeFile(path.join(cwd, "src/main.mts"), "");
    await signalled;
    // The selection waits behind the reload, so it runs after the abort.
    run.abort();
    await run.release();
    await run.running;

    expect(run.bundler.watches).toHaveLength(1);
  });

  it("opens no session when shutdown starts while the old one closes", async () => {
    const cwd = await makeSourceApp("main.js");
    const run = await startReady(cwd);
    let release!: () => void;
    run.bundler.holdClose = new Promise((resolve) => (release = resolve));

    await writeFile(path.join(cwd, "src/main.mts"), "");
    await waitFor(() => expect(run.bundler.closeCalls).toBe(1));
    run.abort();
    release();
    await run.running;

    expect(run.bundler.watches).toHaveLength(1);
  });

  it("ignores the callbacks of a replaced watcher", async () => {
    const cwd = await makeSourceApp("main.js");
    const run = await startReady(cwd);
    await writeFile(path.join(cwd, "src/main.mts"), "");
    await waitFor(() => expect(run.bundler.watches).toHaveLength(2));

    const changed = { file: path.join(cwd, "src/main.mts"), at: 0 };
    run.bundler.watches[0]!.options.onBuild({ ...BUILD_B, changed });
    run.bundler.watches[0]!.options.onError({ message: "stale failure", stack: null });
    // The host answers in order, so a stale reload would be logged before this one.
    run.bundler.emitBuild({ ...BUILD_B, changed });
    await waitFor(() => expect(run.stdout.text()).toContain("reload src/main.mts"));
    expect(await run.hostLog.lines()).toEqual(["start /unused/a.js", "reload"]);

    expect(run.stderr.text()).not.toContain("stale failure");

    run.abort();
    await run.running;
  });

  it("retries a failed entry switch on the next change", async () => {
    const cwd = await makeSourceApp("main.js");
    const run = await startReady(cwd);

    run.bundler.refuseNextWatch = true;
    await writeFile(path.join(cwd, "src/main.mts"), "");
    await waitFor(() => expect(run.stderr.text()).toContain("build failed: watch refused"));
    expect(run.bundler.watches).toHaveLength(1);
    await fsLag();

    await writeFile(path.join(cwd, "package.json"), JSON.stringify({ type: "module" }));
    await waitFor(() => expect(run.bundler.watches).toHaveLength(2));
    run.bundler.emitBuild(BUILD_B);
    await hostEvents(run, ["start /unused/a.js", "reload"]);

    run.abort();
    await run.running;
  });

  it("reports a missing entry at start and starts once one exists", async () => {
    const cwd = await makeSourceApp("main.mts");
    // The unnamed app is reported during the first attempt, after its entry was found.
    const stdout = new Writable({
      write(chunk: Buffer, _encoding, callback) {
        if (chunk.toString().includes("running unnamed")) rmSync(path.join(cwd, "src/main.mts"));
        callback();
      },
    });
    const run = await startDev(cwd, { stdout });

    await waitFor(() => expect(run.stderr.text()).toContain("no app entry found"));
    expect(run.bundler.watches).toHaveLength(0);

    await writeFile(path.join(cwd, "src/App.vue"), APP_VUE);
    await waitFor(() => expect(run.bundler.watches).toHaveLength(1));
    expect(run.bundler.watches[0]!.options.entry).toBe(
      path.join(cwd, "node_modules/.inca/entry.mts"),
    );
    run.bundler.emitBuild(BUILD_A);
    await hostEvents(run, ["start /unused/a.js"]);

    run.abort();
    await run.running;
  });

  it("closes the entry monitor on abort", async () => {
    const cwd = await makeSourceApp("App.vue");
    const run = await startReady(cwd);

    run.abort();
    await run.running;

    expect(watches.allClosed()).toBe(true);
  });

  it("closes the entry monitor when the host exits on its own", async () => {
    const cwd = await makeSourceApp("App.vue");
    const run = await startReady(cwd);

    process.kill(await run.hostLog.pid(), "SIGTERM");
    await run.running;

    expect(watches.allClosed()).toBe(true);
    expect(run.bundler.closed).toBe(true);
  });

  it("closes the entry monitor when the signal is already aborted", async () => {
    const cwd = await makeSourceApp("App.vue");
    const controller = new AbortController();
    controller.abort();
    const run = await startDev(cwd, { signal: controller.signal });

    await run.running;

    expect(run.bundler.watches).toHaveLength(1);
    expect(run.bundler.closed).toBe(true);
    expect(watches.allClosed()).toBe(true);
  });

  it("rejects when the entry monitor cannot be armed again", async () => {
    const cwd = await makeSourceApp("main.js");
    const run = await startReady(cwd);

    watches.refuseNext = true;
    await writeFile(path.join(cwd, "src/main.ts"), "");

    await expect(run.running).rejects.toThrow("too many open files");
    await hostEvents(run, ["start /unused/a.js", "stop /unused/a.js"]);
    expect(run.bundler.closed).toBe(true);
    expect(watches.allClosed()).toBe(true);
  });
});
