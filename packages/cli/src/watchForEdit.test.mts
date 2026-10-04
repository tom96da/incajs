// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { EventEmitter } from "node:events";
import { watch } from "node:fs";
import { mkdir, mkdtemp, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import type { FSWatcher } from "node:fs";

import { afterEach, describe, expect, it, vi } from "vitest";

import { watchForEdit } from "./watchForEdit.mts";
import type { EditWatch } from "./watchForEdit.mts";

vi.mock("node:fs", async (importOriginal) => {
  const actual = await importOriginal<typeof import("node:fs")>();
  return { ...actual, watch: vi.fn<typeof actual.watch>(actual.watch) };
});

const DEBOUNCE = 30;
const SETTLE_MS = 3000;

let root: string | undefined;
const open: EditWatch[] = [];

afterEach(async () => {
  vi.useRealTimers();
  for (const watcher of open.splice(0)) watcher.close();
  if (root) await rm(root, { recursive: true, force: true });
  root = undefined;
});

/** An empty app directory with `src/` and `node_modules/`. */
async function makeApp(): Promise<string> {
  root = await mkdtemp(path.join(tmpdir(), "inca-watch-"));
  await mkdir(path.join(root, "src"));
  await mkdir(path.join(root, "node_modules"));
  return root;
}

/** Arms a watch that the test closes on its way out. */
function arm(cwd: string): EditWatch {
  const watcher = watchForEdit(cwd, { debounceMs: DEBOUNCE });
  open.push(watcher);
  return watcher;
}

/** Whether `promise` resolves within `ms`. */
function settled(promise: Promise<void>, ms: number): Promise<boolean> {
  return Promise.race([promise.then(() => true), sleep(ms).then(() => false)]);
}

/** A stand-in for the watcher `fs.watch` returns. */
function fakeWatcher(): FSWatcher & { close: ReturnType<typeof vi.fn<() => void>> } {
  return Object.assign(new EventEmitter(), {
    close: vi.fn<() => void>(),
  }) as unknown as FSWatcher & {
    close: ReturnType<typeof vi.fn<() => void>>;
  };
}

describe("watchForEdit", () => {
  it.each([
    ["inca.config.ts is written", "inca.config.ts"],
    ["inca.config.mjs is written", "inca.config.mjs"],
    ["package.json is written", "package.json"],
  ])("resolves once %s", async (_, file) => {
    const app = await makeApp();
    const watcher = arm(app);

    await writeFile(path.join(app, file), "{}");

    expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
  });

  it("resolves once a config is renamed into place", async () => {
    const app = await makeApp();
    await writeFile(path.join(app, "draft"), "export default {};");
    const watcher = arm(app);

    await rename(path.join(app, "draft"), path.join(app, "inca.config.ts"));

    expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
  });

  it("resolves once a config is deleted", async () => {
    const app = await makeApp();
    await writeFile(path.join(app, "inca.config.ts"), "export default {};");
    const watcher = arm(app);

    await rm(path.join(app, "inca.config.ts"));

    expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
  });

  it.each(["main.mts", "App.vue"])("resolves once src/%s exists", async (file) => {
    const app = await makeApp();
    const watcher = arm(app);

    await writeFile(path.join(app, "src", file), "");

    expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
  });

  it.each(["main.mts", "App.vue"])(
    "resolves once src/%s is created along with a src/ that was absent",
    async (file) => {
      const app = await makeApp();
      await rm(path.join(app, "src"), { recursive: true });
      const watcher = arm(app);

      await mkdir(path.join(app, "src"));
      await writeFile(path.join(app, "src", file), "");

      expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
    },
  );

  it("ignores every other file", async () => {
    const app = await makeApp();
    await mkdir(path.join(app, "node_modules/.inca"));
    const watcher = arm(app);

    await writeFile(path.join(app, "README.md"), "");
    await writeFile(path.join(app, "src", "util.mts"), "");
    await writeFile(path.join(app, "src", "Other.vue"), "");
    await writeFile(path.join(app, "node_modules/.inca", "entry.mts"), "");
    await writeFile(path.join(app, "node_modules", "package.json"), "{}");

    expect(await settled(watcher.edited, DEBOUNCE * 3)).toBe(false);

    // The watchers are alive, so a write that counts is seen.
    await writeFile(path.join(app, "inca.config.ts"), "");
    expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
  });

  it("resolves once src/ is deleted and created again", async () => {
    const app = await makeApp();
    const watcher = arm(app);

    await rm(path.join(app, "src"), { recursive: true });
    await mkdir(path.join(app, "src"));

    expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
  });

  it("watches a missing directory through its nearest existing ancestor", async () => {
    const app = await makeApp();
    const watcher = arm(path.join(app, "nested", "deep"));

    await writeFile(path.join(app, "unrelated.txt"), "");
    expect(await settled(watcher.edited, DEBOUNCE * 3)).toBe(false);

    await mkdir(path.join(app, "nested", "deep"), { recursive: true });
    expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
  });

  it("keeps an edit made before the caller awaits", async () => {
    const app = await makeApp();
    const watcher = arm(app);

    await writeFile(path.join(app, "inca.config.ts"), "");
    await sleep(DEBOUNCE * 4);

    expect(await settled(watcher.edited, 50)).toBe(true);
  });

  it("waits for a burst of edits to go quiet", async () => {
    vi.useFakeTimers();
    const root = fakeWatcher();
    vi.mocked(watch)
      .mockImplementationOnce(() => root)
      .mockImplementationOnce(() => fakeWatcher());
    const watcher = arm(await makeApp());
    let resolved = false;
    void watcher.edited.then(() => {
      resolved = true;
    });

    for (let i = 0; i < 3; i++) {
      root.emit("change", "change", "inca.config.ts");
      vi.advanceTimersByTime(DEBOUNCE - 10);
    }
    vi.advanceTimersByTime(9);
    await Promise.resolve();
    expect(resolved).toBe(false);

    vi.advanceTimersByTime(1);
    await Promise.resolve();
    expect(resolved).toBe(true);
  });

  describe("add", () => {
    it("watches a file that exists, and nothing else beside it", async () => {
      const app = await makeApp();
      await writeFile(path.join(app, "icon.icns"), "");
      const watcher = arm(app);
      watcher.add(path.join(app, "icon.icns"));

      await writeFile(path.join(app, "sibling.txt"), "");
      expect(await settled(watcher.edited, DEBOUNCE * 3)).toBe(false);

      await writeFile(path.join(app, "icon.icns"), "changed");
      expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
    });

    it("watches a file in a directory that does not exist yet", async () => {
      const app = await makeApp();
      const watcher = arm(app);
      watcher.add(path.join(app, "assets", "icons", "app.icns"));

      await mkdir(path.join(app, "assets", "icons"), { recursive: true });
      expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
    });

    it("watches a file that does not exist yet", async () => {
      const app = await makeApp();
      const watcher = arm(app);
      watcher.add(path.join(app, "app.icns"));

      await writeFile(path.join(app, "app.icns"), "");

      expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
    });

    it("does nothing after close", async () => {
      const watcher = arm(await makeApp());
      watcher.close();
      const calls = vi.mocked(watch).mock.calls.length;

      watcher.add("/tmp/app.icns");

      expect(vi.mocked(watch).mock.calls.length).toBe(calls);
    });
  });

  describe("close", () => {
    it("leaves `edited` pending and ignores later writes", async () => {
      const app = await makeApp();
      const watcher = arm(app);

      watcher.close();
      await writeFile(path.join(app, "inca.config.ts"), "");

      expect(await settled(watcher.edited, DEBOUNCE * 3)).toBe(false);
    });

    it("cancels a debounce that is already running", async () => {
      vi.useFakeTimers();
      const root = fakeWatcher();
      vi.mocked(watch)
        .mockImplementationOnce(() => root)
        .mockImplementationOnce(() => fakeWatcher());
      const watcher = arm(await makeApp());
      let resolved = false;
      void watcher.edited.then(() => {
        resolved = true;
      });

      root.emit("change", "change", "inca.config.ts");
      expect(vi.getTimerCount()).toBe(1);
      vi.advanceTimersByTime(DEBOUNCE / 3);
      watcher.close();
      expect(vi.getTimerCount()).toBe(0);
      vi.advanceTimersByTime(DEBOUNCE * 10);
      await Promise.resolve();

      expect(resolved).toBe(false);
    });

    it("is safe to call twice", async () => {
      const app = await makeApp();
      const watcher = arm(app);

      watcher.close();

      expect(() => watcher.close()).not.toThrow();
    });
  });

  describe("with watchers the operating system controls", () => {
    it("throws when a watcher cannot be created, and closes the ones it made", async () => {
      const app = await makeApp();
      const first = fakeWatcher();
      vi.mocked(watch)
        .mockImplementationOnce(() => first)
        .mockImplementationOnce(() => {
          throw new Error("EMFILE: too many open files");
        });

      expect(() => watchForEdit(app)).toThrow("EMFILE");
      expect(first.close).toHaveBeenCalledTimes(1);
    });

    it("counts an event that names no file", async () => {
      const app = await makeApp();
      const root = fakeWatcher();
      vi.mocked(watch)
        .mockImplementationOnce(() => root)
        .mockImplementationOnce(() => fakeWatcher());
      const watcher = arm(app);

      root.emit("change", "rename", null);

      expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
    });

    it("counts a change to src in the root directory", async () => {
      const app = await makeApp();
      const root = fakeWatcher();
      vi.mocked(watch)
        .mockImplementationOnce(() => root)
        .mockImplementationOnce(() => fakeWatcher());
      const watcher = arm(app);

      root.emit("change", "rename", "src");

      expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
    });

    it("closes only the watcher that reported an error", async () => {
      const app = await makeApp();
      const rootWatcher = fakeWatcher();
      const srcWatcher = fakeWatcher();
      vi.mocked(watch)
        .mockImplementationOnce(() => rootWatcher)
        .mockImplementationOnce(() => srcWatcher);
      arm(app);

      srcWatcher.emit("error", new Error("ENOSPC"));

      expect(srcWatcher.close).toHaveBeenCalledTimes(1);
      expect(rootWatcher.close).not.toHaveBeenCalled();
    });

    it("counts a watcher error as an edit", async () => {
      const app = await makeApp();
      const srcWatcher = fakeWatcher();
      vi.mocked(watch)
        .mockImplementationOnce(() => fakeWatcher())
        .mockImplementationOnce(() => srcWatcher);
      const watcher = arm(app);

      srcWatcher.emit("error", new Error("ENOSPC"));

      expect(await settled(watcher.edited, SETTLE_MS)).toBe(true);
    });
  });
});
