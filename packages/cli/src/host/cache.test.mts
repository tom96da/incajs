// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  truncateSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { PassThrough } from "node:stream";
import { styleText } from "node:util";

import { beforeEach, describe, expect, it, onTestFinished } from "vitest";

import { regularFile, temp, writeHostPackage } from "../../tests/support/hostPackage.mts";
import { cacheRoots, unpackHost } from "./cache.mts";
import type { IncaError } from "../error.mts";
import type { CacheRoot } from "./cache.mts";
import type { HostPackage, TargetKey } from "./package.mts";

const KEY = "linux-x64";
const ENTRY = `${KEY}@1.2.3`;

let binary: Buffer;

beforeEach(() => {
  binary = Buffer.concat([Buffer.from("#!/bin/sh\nexit 0\n"), randomBytes(64)]);
});

/** A host package of the fake binary in a new temporary directory. */
function pkg(hash?: string) {
  return writeHostPackage(path.join(temp(), "pkg"), binary, hash);
}

/** Creates the entry directory `name` in `root`, modified `age` seconds ago. */
function oldEntry(root: string, name: string, age: number): string {
  const dir = path.join(root, name);
  mkdirSync(dir, { recursive: true });
  const time = new Date(Date.now() - age * 1000);
  utimesSync(dir, time, time);
  return dir;
}

/** The mount that forbids running files, when this machine has one. */
function findNoexecDir(): string | undefined {
  try {
    const mounts = readFileSync("/proc/mounts", "utf8").split("\n");
    const line = mounts.find((row) => row.split(" ")[1] === "/dev/shm");
    return line?.split(" ")[3]?.split(",").includes("noexec") ? "/dev/shm" : undefined;
  } catch {
    return undefined;
  }
}

/** `unpackHost` with its unpack line sent to `stderr`. */
function unpack(
  host: HostPackage,
  target: TargetKey,
  roots: readonly CacheRoot[],
  stderr: NodeJS.WritableStream = new PassThrough(),
): Promise<string> {
  return unpackHost(host, target, roots, stderr);
}

/** The error `fn` rejects with. */
async function thrown(fn: () => Promise<unknown>): Promise<IncaError> {
  try {
    await fn();
  } catch (error) {
    return error as IncaError;
  }
  throw new Error("did not throw");
}

describe("cacheRoots", () => {
  const project = { dir: "/app/node_modules/.inca/host", mustExist: "/app/node_modules" };

  it.each([
    [
      "/x/y",
      "linux",
      undefined,
      "/home/u",
      [{ dir: "/x/y" }, { dir: "/home/u/.cache/incajs/host" }],
    ],
    [
      "rel/dir",
      "linux",
      undefined,
      "/home/u",
      [{ dir: path.resolve("rel/dir") }, { dir: "/home/u/.cache/incajs/host" }],
    ],
    ["", "linux", undefined, "/home/u", [{ dir: "/home/u/.cache/incajs/host" }]],
    [undefined, "linux", "/xdg", "/home/u", [{ dir: "/xdg/incajs/host" }]],
    [undefined, "linux", "rel", "/home/u", [{ dir: "/home/u/.cache/incajs/host" }]],
    [undefined, "darwin", undefined, "/home/u", [{ dir: "/home/u/Library/Caches/incajs/host" }]],
    [undefined, "darwin", "/xdg", "/home/u", [{ dir: "/home/u/Library/Caches/incajs/host" }]],
    [
      "/home/u/.cache/incajs/host",
      "linux",
      undefined,
      "/home/u",
      [{ dir: "/home/u/.cache/incajs/host" }],
    ],
    [undefined, "linux", undefined, undefined, []],
    [undefined, "linux", "/xdg", undefined, [{ dir: "/xdg/incajs/host" }]],
    [undefined, "darwin", "/xdg", undefined, []],
  ] as const)(
    "lists the roots for INCA_CACHE_DIR=%j on %s with XDG_CACHE_HOME=%j and home %j",
    (override, platform, xdg, home, expected) => {
      const env = {
        ...(override === undefined ? {} : { INCA_CACHE_DIR: override }),
        ...(xdg === undefined ? {} : { XDG_CACHE_HOME: xdg }),
      };

      expect(cacheRoots(env, platform, home, "/app")).toEqual([...expected, project]);
    },
  );
});

describe("unpackHost", () => {
  it("unpacks a package into an entry named by target and version", async () => {
    const root = temp();

    const result = await unpack(pkg(), KEY, [{ dir: root }]);

    expect(result).toBe(path.join(root, ENTRY, "inca-host"));
    expect(readFileSync(result)).toEqual(binary);
  });

  it("writes one line that names the directory it unpacked into", async () => {
    const stderr = new PassThrough();

    const file = await unpack(pkg(), KEY, [{ dir: temp() }], stderr);

    expect(String(stderr.read())).toBe(
      `${styleText(["bold", "cyan"], "[inca]", { stream: stderr })} unpacked the host binary into ${path.dirname(file)}\n`,
    );
  });

  it("writes nothing when a usable entry exists", async () => {
    const host = pkg();
    const roots = [{ dir: temp() }];
    await unpack(host, KEY, roots);
    const stderr = new PassThrough();

    await unpack(host, KEY, roots, stderr);

    expect(stderr.read()).toBeNull();
  });

  it("names the root that took the file when an earlier root failed", async () => {
    const stderr = new PassThrough();
    const b = temp();

    await unpack(pkg(), KEY, [{ dir: regularFile() }, { dir: b }], stderr);

    expect(String(stderr.read())).toBe(
      `${styleText(["bold", "cyan"], "[inca]", { stream: stderr })} unpacked the host binary into ${path.join(b, ENTRY)}\n`,
    );
  });

  it("makes the unpacked file executable", async () => {
    const file = await unpack(pkg(), KEY, [{ dir: temp() }]);

    expect(statSync(file).mode & 0o777).toBe(0o755);
  });

  it("creates the entry directory with mode 0700", async () => {
    const file = await unpack(pkg(), KEY, [{ dir: temp() }]);

    expect(statSync(path.dirname(file)).mode & 0o777).toBe(0o700);
  });

  it("leaves no temporary file behind", async () => {
    const file = await unpack(pkg(), KEY, [{ dir: temp() }]);

    expect(readdirSync(path.dirname(file))).toEqual(["inca-host"]);
  });

  it("returns a usable entry without reading the package", async () => {
    const [a, b] = [temp(), path.join(temp(), "b")];
    const host = pkg();
    const first = await unpack(host, KEY, [{ dir: a }, { dir: b }]);
    rmSync(path.join(host.dir, "bin/inca-host.br"));

    expect(await unpack(host, KEY, [{ dir: a }, { dir: b }])).toEqual(first);
    expect(existsSync(b)).toBe(false);
  });

  it("returns a usable entry of a later root", async () => {
    const [a, b] = [path.join(temp(), "a"), temp()];
    const host = pkg();
    await unpack(host, KEY, [{ dir: b }]);
    rmSync(path.join(host.dir, "bin/inca-host.br"));

    expect(await unpack(host, KEY, [{ dir: a }, { dir: b }])).toBe(
      path.join(b, ENTRY, "inca-host"),
    );
  });

  it("creates nothing in an earlier root when a later one holds a usable entry", async () => {
    const [a, b] = [path.join(temp(), "a"), temp()];
    const host = pkg();
    await unpack(host, KEY, [{ dir: b }]);

    await unpack(host, KEY, [{ dir: a }, { dir: b }]);

    expect(existsSync(a)).toBe(false);
  });

  it("leaves later roots alone when the first root takes the file", async () => {
    const [a, b] = [temp(), path.join(temp(), "b")];

    expect(await unpack(pkg(), KEY, [{ dir: a }, { dir: b }])).toBe(
      path.join(a, ENTRY, "inca-host"),
    );
    expect(existsSync(b)).toBe(false);
  });

  it("unpacks again over a truncated entry", async () => {
    const host = pkg();
    const roots = [{ dir: temp() }];
    const file = await unpack(host, KEY, roots);
    truncateSync(file, 3);

    await unpack(host, KEY, roots);

    expect(readFileSync(file)).toEqual(binary);
  });

  it("unpacks again over an entry that isn't executable", async () => {
    const host = pkg();
    const roots = [{ dir: temp() }];
    const file = await unpack(host, KEY, roots);
    chmodSync(file, 0o644);

    await unpack(host, KEY, roots);

    expect(statSync(file).mode & 0o777).toBe(0o755);
  });

  it("replaces an entry that a process is running", async () => {
    const root = temp();
    const file = path.join(root, ENTRY, "inca-host");
    mkdirSync(path.dirname(file), { recursive: true });
    copyFileSync("/bin/sleep", file);
    chmodSync(file, 0o755);
    const child = spawn(file, ["5"]);
    try {
      await new Promise((resolve) => child.once("spawn", resolve));

      await unpack(pkg(), KEY, [{ dir: root }]);

      expect(readFileSync(file)).toEqual(binary);
    } finally {
      child.kill();
    }
  });

  it("uses the next root when the first can't be written", async () => {
    const a = regularFile();
    const b = temp();

    const result = await unpack(pkg(), KEY, [{ dir: a }, { dir: b }]);

    expect(result).toBe(path.join(b, ENTRY, "inca-host"));
    expect(readFileSync(result)).toEqual(binary);
    expect(statSync(a).isFile()).toBe(true);
  });

  it("unpacks into the project root when node_modules exists", async () => {
    const cwd = temp();
    mkdirSync(path.join(cwd, "node_modules"));
    const [project] = cacheRoots({}, "linux", undefined, cwd);

    const result = await unpack(pkg(), KEY, [{ dir: regularFile() }, project!]);

    const root = path.join(cwd, "node_modules/.inca/host");
    expect(result).toBe(path.join(root, ENTRY, "inca-host"));
  });

  it("doesn't create node_modules for the project root", async () => {
    const cwd = temp();
    const roots = cacheRoots({}, "linux", undefined, cwd);

    const error = await thrown(() => unpack(pkg(), KEY, roots));

    expect(error.code).toBe("ERR_INCA_HOST_UNPACK_FAILED");
    expect(existsSync(path.join(cwd, "node_modules"))).toBe(false);
  });

  it("treats a node_modules that is a file as missing", async () => {
    const cwd = temp();
    writeFileSync(path.join(cwd, "node_modules"), "x");
    const roots = cacheRoots({}, "linux", undefined, cwd);

    const error = await thrown(() => unpack(pkg(), KEY, roots));

    expect(error.code).toBe("ERR_INCA_HOST_UNPACK_FAILED");
    expect(readFileSync(path.join(cwd, "node_modules"), "utf8")).toBe("x");
  });

  it.skipIf(process.getuid?.() === 0)(
    "treats a project directory it can't search as missing",
    async () => {
      const cwd = temp();
      mkdirSync(path.join(cwd, "node_modules"));
      const roots = cacheRoots({}, "linux", undefined, cwd);
      const host = pkg();
      chmodSync(cwd, 0o000);

      const error = await thrown(() => unpack(host, KEY, roots));

      expect(error.code).toBe("ERR_INCA_HOST_UNPACK_FAILED");
    },
  );

  it("tells the user to set the cache directory when no root takes the file", async () => {
    const roots = [{ dir: regularFile() }];

    const error = await thrown(() => unpack(pkg(), KEY, roots));

    expect(error.code).toBe("ERR_INCA_HOST_UNPACK_FAILED");
    expect(error.message).toContain("set INCA_CACHE_DIR to a writable directory");
  });

  it("throws a corrupt-package error when the compressed file is missing", async () => {
    const [a, b] = [temp(), path.join(temp(), "b")];
    const host = pkg();
    rmSync(path.join(host.dir, "bin/inca-host.br"));

    const error = await thrown(() => unpack(host, KEY, [{ dir: a }, { dir: b }]));

    expect(error.code).toBe("ERR_INCA_HOST_PACKAGE_CORRUPT");
    expect(error.message).toContain("bin/inca-host.br");
    expect(existsSync(b)).toBe(false);
  });

  it("throws a corrupt-package error when the compressed file is not brotli", async () => {
    const host = pkg();
    writeFileSync(path.join(host.dir, "bin/inca-host.br"), Buffer.from("not brotli"));

    const error = await thrown(() => unpack(host, KEY, [{ dir: temp() }]));

    expect(error.code).toBe("ERR_INCA_HOST_PACKAGE_CORRUPT");
    expect(error.message).toContain("can't be unpacked");
  });

  it("throws a corrupt-package error when the bytes don't match the recorded hash", async () => {
    const error = await thrown(() => unpack(pkg("0".repeat(64)), KEY, [{ dir: temp() }]));

    expect(error.code).toBe("ERR_INCA_HOST_PACKAGE_CORRUPT");
    expect(error.message).toContain("verification failed");
  });

  it("reports an unpack failure without reading the package when no root is tried", async () => {
    const host = pkg();
    rmSync(path.join(host.dir, "bin/inca-host.br"));
    const roots = cacheRoots({}, "linux", undefined, temp());

    expect((await thrown(() => unpack(host, KEY, roots))).code).toBe("ERR_INCA_HOST_UNPACK_FAILED");
  });

  it("removes its temporary file when the rename fails", async () => {
    const [a, b] = [temp(), temp()];
    const blocker = path.join(a, ENTRY, "inca-host");
    mkdirSync(blocker, { recursive: true });
    writeFileSync(path.join(blocker, "keep"), "x");

    await unpack(pkg(), KEY, [{ dir: a }, { dir: b }]);

    expect(readdirSync(path.join(a, ENTRY))).toEqual(["inca-host"]);
    expect(readdirSync(blocker)).toEqual(["keep"]);
  });

  it("doesn't treat a stale temporary file as the entry", async () => {
    const root = temp();
    const stale = path.join(root, ENTRY, "inca-host.tmp-1-aaaaaaaa");
    mkdirSync(path.dirname(stale), { recursive: true });
    writeFileSync(stale, "abc");

    const file = await unpack(pkg(), KEY, [{ dir: root }]);

    expect(readFileSync(file)).toEqual(binary);
    expect(readFileSync(stale, "utf8")).toBe("abc");
  });

  const noexecDir = findNoexecDir();
  it.runIf(noexecDir !== undefined)(
    "falls back past a root on a mount that forbids running files",
    async () => {
      const a = path.join(noexecDir!, `inca-test-${process.pid}`);
      onTestFinished(() => rmSync(a, { recursive: true, force: true }));
      mkdirSync(a, { recursive: true });
      const b = temp();

      const result = await unpack(pkg(), KEY, [{ dir: a }, { dir: b }]);

      expect(result).toBe(path.join(b, ENTRY, "inca-host"));
      expect(readdirSync(path.join(a, ENTRY))).toEqual([]);
    },
  );

  it("keeps the three newest entries of a target in the root it used", async () => {
    const root = temp();
    const old = [1, 2, 3, 4, 5].map((age) => oldEntry(root, `${KEY}@0.0.${age}`, age * 10));

    await unpack(pkg(), KEY, [{ dir: root }]);

    expect(readdirSync(root).sort()).toEqual(["linux-x64@0.0.1", "linux-x64@0.0.2", ENTRY]);
    expect(old.map((dir) => existsSync(dir))).toEqual([true, true, false, false, false]);
  });

  it("leaves other names in the root alone", async () => {
    const root = temp();
    for (const [age, v] of [
      [10, 3],
      [20, 4],
      [30, 5],
    ] as const)
      oldEntry(root, `${KEY}@0.0.${v}`, age);
    const decoys = [
      oldEntry(root, "linux-arm64@0.0.1", 900),
      oldEntry(root, `${KEY}@notes`, 900),
      oldEntry(root, `${KEY}@0.0.1x`, 900),
      oldEntry(root, "notes", 900),
    ];
    const fileDecoy = path.join(root, `${KEY}@0.0.2`);
    writeFileSync(fileDecoy, "x");
    const time = new Date(Date.now() - 900_000);
    utimesSync(fileDecoy, time, time);

    await unpack(pkg(), KEY, [{ dir: root }]);

    expect([...decoys, fileDecoy].map((item) => existsSync(item))).toEqual(Array(5).fill(true));
    expect(existsSync(path.join(root, `${KEY}@0.0.5`))).toBe(false);
  });

  it("prunes only inside node_modules/.inca/host", async () => {
    const cwd = temp();
    const inca = path.join(cwd, "node_modules/.inca");
    mkdirSync(path.join(inca, "config"), { recursive: true });
    writeFileSync(path.join(inca, "dev.pid"), "1");
    writeFileSync(path.join(inca, "entry.mts"), "x");
    const host = path.join(inca, "host");
    const old = [1, 2, 3, 4].map((age) => oldEntry(host, `${KEY}@0.0.${age}`, age * 10));

    await unpack(pkg(), KEY, cacheRoots({}, "linux", undefined, cwd));

    expect(old.map((dir) => existsSync(dir))).toEqual([true, true, false, false]);
    expect(existsSync(path.join(host, ENTRY))).toBe(true);
    expect(readdirSync(inca).sort()).toEqual(["config", "dev.pid", "entry.mts", "host"]);
  });

  it("doesn't prune a root it didn't use", async () => {
    const [a, b] = [temp(), temp()];
    const blocker = path.join(a, ENTRY, "inca-host");
    mkdirSync(blocker, { recursive: true });
    writeFileSync(path.join(blocker, "keep"), "x");
    const old = [1, 2, 3, 4].map((age) => oldEntry(a, `${KEY}@0.0.${age}`, age * 10 + 100));

    expect(await unpack(pkg(), KEY, [{ dir: a }, { dir: b }])).toBe(
      path.join(b, ENTRY, "inca-host"),
    );
    expect(old.map((dir) => existsSync(dir))).toEqual([true, true, true, true]);
  });

  it.skipIf(process.getuid?.() === 0)("ignores a directory it can't remove", async () => {
    const root = temp();
    const oldest = oldEntry(root, `${KEY}@0.0.3`, 300);
    oldEntry(root, `${KEY}@0.0.2`, 200);
    oldEntry(root, `${KEY}@0.0.1`, 100);
    const locked = path.join(oldest, "sub");
    mkdirSync(locked);
    writeFileSync(path.join(locked, "file"), "x");
    chmodSync(locked, 0o500);
    const aged = new Date(Date.now() - 300_000);
    utimesSync(oldest, aged, aged);
    try {
      await expect(unpack(pkg(), KEY, [{ dir: root }])).resolves.toBeTypeOf("string");
      expect(existsSync(path.join(locked, "file"))).toBe(true);
    } finally {
      chmodSync(locked, 0o700);
    }
  });
});
