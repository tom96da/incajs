// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Launches the real `inca-host` the way a packaged app starts: no argv, a
// working directory unrelated to the app, and the bundle found beside the
// executable. The bundles create no window, so these cases need no display
// on any platform.

import { spawn } from "node:child_process";
import {
  cp,
  link,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { resolveTestHostBin } from "../support/hostBin.mts";

const TIMEOUT_MS = 15_000;

const hostBin = resolveTestHostBin();

let dir: string;

beforeEach(async () => {
  // The host reports the executable's resolved path, so compare against that.
  dir = await realpath(await mkdtemp(path.join(tmpdir(), "inca-host-packaged-")));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

/** Puts the host at `exeDir/app`; hard-linked so `current_exe` stays in `exeDir`. */
async function placeExecutable(exeDir: string): Promise<string> {
  await mkdir(exeDir, { recursive: true });
  const exe = path.join(exeDir, "app");
  await link(hostBin, exe).catch(() => cp(hostBin, exe));
  return exe;
}

/** Runs `exe` with no argv from `/`, resolving with its exit code and output. */
function launch(
  exe: string,
  env?: NodeJS.ProcessEnv,
): Promise<{ code: number | null; stdout: string; stderr: string }> {
  const child = spawn(exe, [], { cwd: "/", stdio: ["ignore", "pipe", "pipe"], env });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8").on("data", (chunk: string) => (stdout += chunk));
  child.stderr.setEncoding("utf8").on("data", (chunk: string) => (stderr += chunk));
  return new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", (code) => resolve({ code, stdout, stderr }));
  });
}

const THROWING_BUNDLE = 'console.log("marker");\nthrow new Error("boom");\n';

/** The environment that keeps the log of a run inside `home`. */
const homeEnv = (home: string): NodeJS.ProcessEnv => ({
  ...process.env,
  HOME: home,
  XDG_STATE_HOME: home,
});

describe("a packaged app launched with no arguments", () => {
  it(
    "runs the bundle beside the executable (flat layout)",
    async () => {
      const exe = await placeExecutable(path.join(dir, "app"));
      const bundle = path.join(dir, "app", "bundle.js");
      await writeFile(bundle, THROWING_BUNDLE);

      const { code, stdout, stderr } = await launch(exe, homeEnv(dir));

      expect(code).toBe(1);
      expect(stdout).toBe("");
      expect(stderr).toMatch(/^marker\nError: boom\n/);
      expect(stderr).toContain(bundle);
    },
    TIMEOUT_MS,
  );

  it(
    "runs the bundle under Contents/Resources (.app layout)",
    async () => {
      const exe = await placeExecutable(path.join(dir, "App.app", "Contents", "MacOS"));
      const resources = path.join(dir, "App.app", "Contents", "Resources");
      await mkdir(resources, { recursive: true });
      await writeFile(path.join(resources, "bundle.js"), THROWING_BUNDLE);

      const { code, stdout, stderr } = await launch(exe, homeEnv(dir));

      expect(code).toBe(1);
      expect(stdout).toBe("");
      expect(stderr).toMatch(/^marker\nError: boom\n/);
    },
    TIMEOUT_MS,
  );

  it(
    "a throwing first bundle leaves a log file with the error",
    async () => {
      const exe = await placeExecutable(path.join(dir, "app"));
      await writeFile(path.join(dir, "app", "bundle.js"), THROWING_BUNDLE);
      const logDir =
        process.platform === "darwin"
          ? path.join(dir, "Library", "Logs", "inca-app")
          : path.join(dir, "inca-app");

      const { stderr } = await launch(exe, homeEnv(dir));

      expect(stderr).not.toContain("run log");
      const files = await readdir(logDir);
      expect(files).toHaveLength(1);
      expect(files[0]).toMatch(/^\d{8}T\d{6}Z\.log$/);
      const lines = (await readFile(path.join(logDir, files[0] ?? ""), "utf8")).split("\n");
      expect(lines[0]).toMatch(/^(@incajs\/host-\S+|inca-host \S+\/\S+) \S+$/);
      expect(lines[1]).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z startup$/);
      expect(lines[2]).toBe("Error: boom");
    },
    TIMEOUT_MS,
  );

  it(
    "exits 1 naming the missing bundle when none is beside the executable",
    async () => {
      const exe = await placeExecutable(path.join(dir, "app"));

      const { code, stdout, stderr } = await launch(exe);

      expect(code).toBe(1);
      expect(stdout).toBe("");
      expect(stderr).toContain("no bundle.js found beside the executable");
    },
    TIMEOUT_MS,
  );
});
