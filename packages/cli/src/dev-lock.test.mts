// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { spawn } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { acquireDevLock } from "./dev-lock.mts";

let cwd: string | undefined;
const releases: (() => Promise<void>)[] = [];

afterEach(async () => {
  for (const release of releases.splice(0)) await release();
  if (cwd) await rm(cwd, { recursive: true, force: true });
  cwd = undefined;
});

/**
 * Creates a scratch app directory, optionally with a lock file already in place.
 *
 * @param pid - the pid text to record in the lock file, or none for no lock file
 * @returns the app directory and the lock file's path
 */
async function makeApp(pid?: string): Promise<{ app: string; file: string }> {
  const app = await mkdtemp(path.join(tmpdir(), "inca-dev-lock-"));
  cwd = app;
  const file = path.join(app, "node_modules/.inca/dev.pid");
  if (pid !== undefined) {
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, pid);
  }
  return { app, file };
}

describe("acquireDevLock", () => {
  it("takes over a lock file holding this process's own pid", async () => {
    const { app, file } = await makeApp(String(process.pid));

    releases.push(await acquireDevLock(app));

    expect(await readFile(file, "utf8")).toContain(String(process.pid));
  });

  it("takes over a lock file whose pid is gone", async () => {
    const child = spawn(process.execPath, ["-e", ""], { stdio: "ignore" });
    await new Promise((resolve) => child.once("exit", resolve));
    const { app, file } = await makeApp(String(child.pid));

    releases.push(await acquireDevLock(app));

    expect(await readFile(file, "utf8")).toContain(String(process.pid));
  });

  it("rejects while another live process holds the lock", async () => {
    const { app } = await makeApp(String(process.ppid));

    await expect(acquireDevLock(app)).rejects.toMatchObject({ code: "ERR_INCA_DEV_RUNNING" });
  });

  it("lets exactly one of two concurrent calls succeed", async () => {
    const { app } = await makeApp();

    const results = await Promise.allSettled([acquireDevLock(app), acquireDevLock(app)]);

    for (const result of results) {
      if (result.status === "fulfilled") releases.push(result.value);
    }
    expect(results.filter((r) => r.status === "fulfilled")).toHaveLength(1);
    const rejected = results.find((r) => r.status === "rejected");
    expect(rejected?.reason).toMatchObject({ code: "ERR_INCA_DEV_RUNNING" });
  });

  it("removes the lock file on release", async () => {
    const { app, file } = await makeApp();

    await (
      await acquireDevLock(app)
    )();

    await expect(readFile(file, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  });

  it("leaves a successor's lock alone when a release is called twice", async () => {
    const { app, file } = await makeApp();
    const first = await acquireDevLock(app);
    await first();
    releases.push(await acquireDevLock(app));

    await first();

    expect(await readFile(file, "utf8")).toContain(String(process.pid));
  });

  it("takes over an empty or unparsable lock file", async () => {
    const { app, file } = await makeApp("");
    for (const text of ["", "junk"]) {
      await writeFile(file, text);
      releases.push(await acquireDevLock(app));
      expect(await readFile(file, "utf8")).toContain(String(process.pid));
    }
  });

  it("takes over a lock file holding pid 0", async () => {
    const { app, file } = await makeApp("0");

    releases.push(await acquireDevLock(app));

    expect(await readFile(file, "utf8")).toContain(String(process.pid));
  });

  it("refuses a relative form of a directory this process holds", async () => {
    const { app } = await makeApp();
    releases.push(await acquireDevLock(app));

    await expect(acquireDevLock(path.relative(process.cwd(), app))).rejects.toMatchObject({
      code: "ERR_INCA_DEV_RUNNING",
    });
  });
});
