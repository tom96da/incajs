// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { watch } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { Writable } from "node:stream";
import { setTimeout as sleep } from "node:timers/promises";

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MockInstance } from "vitest";

import { IncaError } from "./error.mts";
import { retryOnEdit } from "./retryOnEdit.mts";
import type { RetryOnEditOptions } from "./retryOnEdit.mts";
import type { EditWatch } from "./watchForEdit.mts";

vi.mock("node:fs", async (importOriginal) => {
  const actual = await importOriginal<typeof import("node:fs")>();
  return { ...actual, watch: vi.fn<typeof actual.watch>(actual.watch) };
});

const DEBOUNCE = 30;

let app: string;
let controller: AbortController;

beforeEach(async () => {
  app = await mkdtemp(path.join(tmpdir(), "inca-retry-"));
  controller = new AbortController();
});

afterEach(async () => {
  controller.abort();
  await rm(app, { recursive: true, force: true });
});

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

/** How many times `needle` appears in `text`. */
function count(text: string, needle: string): number {
  return text.split(needle).length - 1;
}

function touchConfig(): Promise<void> {
  return writeFile(path.join(app, "inca.config.ts"), String(Math.random()));
}

/** Runs `retryOnEdit` against `attempt`, with sinks the test reads back. */
function start<T>(attempt: RetryOnEditOptions<T>["attempt"]) {
  const stdout = makeSink();
  const stderr = makeSink();
  const run = vi.fn<RetryOnEditOptions<T>["attempt"]>(attempt);
  const running = retryOnEdit({
    cwd: app,
    signal: controller.signal,
    stdout: stdout.stream,
    stderr: stderr.stream,
    debounceMs: DEBOUNCE,
    attempt: run,
  });
  return { running, run, stdout, stderr };
}

/** Whether `promise` settles within `ms`. */
function settles(promise: Promise<unknown>, ms: number): Promise<boolean> {
  return Promise.race([promise.then(() => true), sleep(ms).then(() => false)]);
}

describe("retryOnEdit", () => {
  it("returns the value of a first attempt that succeeds, in silence", async () => {
    const closes: MockInstance<() => void>[] = [];
    const { running, stdout, stderr } = start((w) => {
      closes.push(vi.spyOn(w, "close"));
      return Promise.resolve(7);
    });

    await expect(running).resolves.toBe(7);

    expect(stdout.text()).toBe("");
    expect(stderr.text()).toBe("");
    expect(closes[0]).toHaveBeenCalled();
  });

  it("prints the failure, says it is waiting, and stays pending", async () => {
    const { running, run, stdout, stderr } = start(() =>
      Promise.reject(new IncaError("ERR_INCA_TEST", "boom")),
    );

    await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

    expect(stderr.text()).toContain("[inca] build failed (ERR_INCA_TEST): boom");
    expect(await settles(running, DEBOUNCE * 3)).toBe(false);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("retries after an edit and says so", async () => {
    let calls = 0;
    const { running, run, stdout } = start(() => {
      calls++;
      return calls === 1 ? Promise.reject(new Error("first")) : Promise.resolve("done");
    });
    await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

    await touchConfig();

    await expect(running).resolves.toBe("done");
    expect(run).toHaveBeenCalledTimes(2);
    expect(count(stdout.text(), "[inca] retrying")).toBe(1);
  });

  it("prints each failure once per edit", async () => {
    const { run, stderr } = start(() => Promise.reject(new Error("still broken")));

    for (let edits = 1; edits <= 3; edits++) {
      await vi.waitFor(() => expect(count(stderr.text(), "build failed")).toBe(edits));
      await touchConfig();
    }

    await vi.waitFor(() => expect(count(stderr.text(), "build failed")).toBe(4));
    expect(run).toHaveBeenCalledTimes(4);
  });

  it("keeps an edit made during the failing attempt", async () => {
    let calls = 0;
    const { running, run } = start(async () => {
      calls++;
      if (calls > 1) return "done";
      await touchConfig();
      throw new Error("failed after the save");
    });

    await expect(running).resolves.toBe("done");
    expect(run).toHaveBeenCalledTimes(2);
  });

  it("ignores an unrelated file written while it waits", async () => {
    const { run, stdout, stderr } = start(() => Promise.reject(new Error("broken")));
    await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

    await writeFile(path.join(app, "README.md"), "");
    await sleep(DEBOUNCE * 4);

    expect(run).toHaveBeenCalledTimes(1);
    expect(count(stderr.text(), "build failed")).toBe(1);

    // The watcher is alive, so a write that counts is seen.
    await touchConfig();
    await vi.waitFor(() => expect(run).toHaveBeenCalledTimes(2));
  });

  it("retries once the file an error names is created", async () => {
    const icon = path.join(app, "app.icns");
    let calls = 0;
    const { running, run, stdout } = start(() => {
      calls++;
      return calls === 1
        ? Promise.reject(new IncaError("ERR_INCA_ICON_NOT_FOUND", "no icon", icon))
        : Promise.resolve("done");
    });
    await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

    await writeFile(icon, "");

    await expect(running).resolves.toBe("done");
    expect(run).toHaveBeenCalledTimes(2);
  });

  it("retries at once and in silence when the file an error names already exists", async () => {
    const icon = path.join(app, "app.icns");
    await writeFile(icon, "");
    let calls = 0;
    const { running, run, stdout, stderr } = start(() => {
      calls++;
      return calls === 1
        ? Promise.reject(new IncaError("ERR_INCA_ICON_NOT_FOUND", "no icon", icon))
        : Promise.resolve("done");
    });

    await expect(running).resolves.toBe("done");

    expect(run).toHaveBeenCalledTimes(2);
    expect(stdout.text()).toBe("");
    expect(stderr.text()).toBe("");
  });

  it("prints only the second failure when the silent retry fails again", async () => {
    const icon = path.join(app, "app.icns");
    await writeFile(icon, "");
    let calls = 0;
    const { run, stdout, stderr } = start(() => {
      calls++;
      return Promise.reject(
        calls === 1 ? new IncaError("ERR_INCA_ICON_NOT_FOUND", "stale", icon) : new Error("second"),
      );
    });

    await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

    expect(run).toHaveBeenCalledTimes(2);
    expect(count(stderr.text(), "build failed")).toBe(1);
    expect(stderr.text()).toContain("build failed: second");
    expect(stderr.text()).not.toContain("stale");
    expect(stdout.text()).not.toContain("[inca] retrying");
  });

  it("waits for an edit after one silent retry when the named file exists", async () => {
    const icon = path.join(app, "app.icns");
    await writeFile(icon, "");
    const { run, stdout, stderr } = start(() =>
      Promise.reject(new IncaError("ERR_INCA_ICON_NOT_FOUND", "no icon", icon)),
    );

    await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

    expect(run).toHaveBeenCalledTimes(2);
    expect(count(stderr.text(), "build failed")).toBe(1);
    expect(stdout.text()).not.toContain("[inca] retrying");
  });

  it("allows a silent retry again after a wait", async () => {
    const icon = path.join(app, "app.icns");
    await writeFile(icon, "");
    let calls = 0;
    const { running, run, stdout, stderr } = start(() => {
      calls++;
      return calls === 4
        ? Promise.resolve("done")
        : Promise.reject(new IncaError("ERR_INCA_ICON_NOT_FOUND", "no icon", icon));
    });
    await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

    await touchConfig();

    await expect(running).resolves.toBe("done");
    expect(run).toHaveBeenCalledTimes(4);
    expect(count(stderr.text(), "build failed")).toBe(1);
    expect(count(stdout.text(), "[inca] retrying")).toBe(1);
  });

  it("prints the failure before it rejects with a watcher error on the waiting path", async () => {
    const real = vi.mocked(watch).getMockImplementation() as typeof watch;
    vi.mocked(watch)
      .mockImplementationOnce(real)
      .mockImplementationOnce(real)
      .mockImplementationOnce(() => {
        throw new Error("EMFILE: too many open files");
      });
    const { running, stderr, stdout } = start(() =>
      Promise.reject(new IncaError("ERR_INCA_ICON_NOT_FOUND", "no icon", path.join(app, "x.icns"))),
    );

    await expect(running).rejects.toThrow("EMFILE");

    expect(stderr.text()).toContain("build failed (ERR_INCA_ICON_NOT_FOUND): no icon");
    expect(stdout.text()).toBe("");
  });

  it("prints the failure before it rejects with a watcher error on the silent path", async () => {
    const icon = path.join(app, "app.icns");
    await writeFile(icon, "");
    const real = vi.mocked(watch).getMockImplementation() as typeof watch;
    vi.mocked(watch)
      .mockImplementationOnce(real)
      .mockImplementationOnce(real)
      .mockImplementationOnce(() => {
        throw new Error("EMFILE: too many open files");
      });
    const { running, run, stderr } = start(() =>
      Promise.reject(new IncaError("ERR_INCA_ICON_NOT_FOUND", "no icon", icon)),
    );

    await expect(running).rejects.toThrow("EMFILE");

    expect(run).toHaveBeenCalledTimes(1);
    expect(stderr.text()).toContain("build failed (ERR_INCA_ICON_NOT_FOUND): no icon");
  });

  it("prints the failure and returns when the signal is aborted and the named file exists", async () => {
    const icon = path.join(app, "app.icns");
    await writeFile(icon, "");
    const { running, run, stdout, stderr } = start(() => {
      controller.abort();
      return Promise.reject(new IncaError("ERR_INCA_ICON_NOT_FOUND", "no icon", icon));
    });

    await expect(running).resolves.toBeUndefined();

    expect(run).toHaveBeenCalledTimes(1);
    expect(stderr.text()).toContain("build failed (ERR_INCA_ICON_NOT_FOUND): no icon");
    expect(stdout.text()).toBe("");
  });

  it("resolves undefined when the signal aborts during the wait", async () => {
    const closes: MockInstance<() => void>[] = [];
    const { running, run, stdout } = start((w) => {
      closes.push(vi.spyOn(w, "close"));
      return Promise.reject(new Error("broken"));
    });
    await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

    controller.abort();
    await expect(running).resolves.toBeUndefined();
    await touchConfig();
    await sleep(DEBOUNCE * 4);

    expect(run).toHaveBeenCalledTimes(1);
    expect(closes[0]).toHaveBeenCalled();
    expect(stdout.text()).not.toContain("[inca] retrying");
  });

  it("resolves undefined at once when the signal is already aborted at a failure", async () => {
    const { running, run, stdout, stderr } = start(() => {
      controller.abort();
      return Promise.reject(new Error("broken"));
    });

    await expect(running).resolves.toBeUndefined();

    expect(run).toHaveBeenCalledTimes(1);
    expect(stderr.text()).toContain("build failed: broken");
    expect(stdout.text()).toBe("");
  });

  it("prints a value that is not an Error", async () => {
    const { stdout, stderr } = start(() => Promise.reject("plain text"));

    await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

    expect(stderr.text()).toContain("[inca] build failed: plain text");
  });

  it("gives each attempt a fresh watch and closes the one before", async () => {
    const watches: EditWatch[] = [];
    const closes: MockInstance<() => void>[] = [];
    let calls = 0;
    const { running, stdout } = start((w) => {
      closes.push(vi.spyOn(w, "close"));
      watches.push(w);
      calls++;
      return calls === 1 ? Promise.reject(new Error("first")) : Promise.resolve("done");
    });
    await vi.waitFor(() => expect(stdout.text()).toContain("waiting for a change to retry"));

    await touchConfig();
    await running;

    expect(watches).toHaveLength(2);
    expect(watches[0]).not.toBe(watches[1]);
    expect(closes[0]).toHaveBeenCalled();
    expect(closes[1]).toHaveBeenCalled();
  });
});
