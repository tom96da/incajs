// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Runs the real `inca-host` binary with `--print-config`, which prints and
// exits before any window opens.

import { spawnSync } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, beforeEach, expect, it } from "vitest";

import { resolveTestHostBin } from "../support/hostBin.mts";

const hostBin = resolveTestHostBin();

let dir: string;

beforeEach(async () => {
  dir = await mkdtemp(path.join(tmpdir(), "inca-host-print-config-"));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

/** Runs the host with `args`, with no display to open a window on. */
function run(args: string[]) {
  return spawnSync(hostBin, args, { encoding: "utf8", timeout: 15_000 });
}

it("prints the settings read from inca.json and exits 0", async () => {
  await writeFile(path.join(dir, "bundle.js"), "throw new Error('never evaluated');");
  await writeFile(
    path.join(dir, "inca.json"),
    JSON.stringify({ name: "Demo", window: { width: 640, resizable: true } }),
  );

  const result = run(["--print-config", path.join(dir, "bundle.js")]);

  expect(result.status).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual({
    name: "Demo",
    identifier: null,
    window: {
      title: "Demo",
      width: 640,
      height: null,
      resizable: true,
      minWidth: null,
      minHeight: null,
    },
  });
});

it("exits 1 naming an entry that is not a file", () => {
  const entry = path.join(dir, "missing.js");

  const result = run(["--print-config", entry]);

  expect(result.status).toBe(1);
  expect(result.stdout).toBe("");
  expect(result.stderr).toContain(`failed to read ${entry}`);
});

it("exits 1 naming the field of inca.json it cannot use", async () => {
  await writeFile(path.join(dir, "bundle.js"), "");
  await writeFile(path.join(dir, "inca.json"), JSON.stringify({ window: { width: "wide" } }));

  const result = run(["--print-config", path.join(dir, "bundle.js")]);

  expect(result.status).toBe(1);
  expect(result.stdout).toBe("");
  const file = path.join(dir, "inca.json");
  expect(result.stderr.startsWith(`failed to read ${file}: ["window","width"]: `)).toBe(true);
  expect(result.stderr.split("\n")).toHaveLength(2);
});

it("exits 1 with the usage when --print-config comes with --dev", async () => {
  await writeFile(path.join(dir, "bundle.js"), "");

  const result = run(["--print-config", "--dev", path.join(dir, "bundle.js")]);

  expect(result.status).toBe(1);
  expect(result.stdout).toBe("");
  expect(result.stderr).toContain("usage: inca-host [--dev | --print-config]");
});
