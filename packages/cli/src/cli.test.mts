// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MockInstance } from "vitest";

vi.mock("./dev.mts", () => ({
  dev: vi.fn<(options: DevOptions) => Promise<void>>(() => new Promise(() => {})),
}));
vi.mock("./build.mts", () => ({
  build: vi.fn<(options?: BuildAppOptions) => Promise<BuildOutput>>(),
}));
vi.mock("./package.mts", () => ({
  packageApp: vi.fn<(options?: PackageAppOptions) => Promise<PackageResult>>(),
}));

import { build } from "./build.mts";
import { run } from "./cli.mts";
import { dev } from "./dev.mts";
import { packageApp } from "./package.mts";
import type { BuildOutput } from "./adapter/types.mts";
import type { BuildAppOptions } from "./build.mts";
import type { DevOptions } from "./dev.mts";
import type { PackageAppOptions, PackageResult } from "./package.mts";

const mockedDev = vi.mocked(dev);
const mockedBuild = vi.mocked(build);
const mockedPackageApp = vi.mocked(packageApp);

let stdout: MockInstance;
let stderr: MockInstance;

function written(spy: MockInstance): string {
  return spy.mock.calls.map((call) => String(call[0])).join("");
}

beforeEach(() => {
  stdout = vi.spyOn(process.stdout, "write").mockImplementation(() => true);
  stderr = vi.spyOn(process.stderr, "write").mockImplementation(() => true);
});

afterEach(() => {
  stdout.mockRestore();
  stderr.mockRestore();
  mockedDev.mockClear();
  mockedBuild.mockClear();
  mockedPackageApp.mockClear();
  process.removeAllListeners("SIGINT");
  process.removeAllListeners("SIGTERM");
  process.exitCode = undefined;
});

describe("run", () => {
  it("prints usage and sets a non-zero exit code for anything but dev/build/package", async () => {
    await run(["node", "inca"]);

    expect(process.exitCode).toBe(1);
    expect(written(stderr)).toContain("Usage: inca <dev|build|package>");
    expect(mockedDev).not.toHaveBeenCalled();
    expect(mockedBuild).not.toHaveBeenCalled();
    expect(mockedPackageApp).not.toHaveBeenCalled();
  });

  it("runs build and leaves the exit code untouched on success", async () => {
    mockedBuild.mockResolvedValue({
      outDir: "/app/dist",
      entryFile: "/app/dist/bundle.js",
      files: ["bundle.js"],
    });

    await run(["node", "inca", "build"]);

    expect(mockedBuild).toHaveBeenCalled();
    expect(mockedDev).not.toHaveBeenCalled();
    expect(process.exitCode).toBeUndefined();
    expect(written(stdout)).toContain("[inca] built /app/dist/bundle.js");
  });

  it("sets a non-zero exit code and prints a readable error when build fails", async () => {
    mockedBuild.mockRejectedValue(new Error("syntax error"));

    await run(["node", "inca", "build"]);

    expect(process.exitCode).toBe(1);
    expect(written(stderr)).toContain("[inca] build failed: syntax error");
  });

  it("runs package and leaves the exit code untouched on success", async () => {
    mockedPackageApp.mockResolvedValue({ appPath: "/app/dist/click_counter.app" });

    await run(["node", "inca", "package"]);

    expect(mockedPackageApp).toHaveBeenCalled();
    expect(process.exitCode).toBeUndefined();
    expect(written(stdout)).toContain("[inca] packaged /app/dist/click_counter.app");
  });

  it("sets a non-zero exit code and prints a readable error when package fails", async () => {
    mockedPackageApp.mockRejectedValue(new Error("no host binary"));

    await run(["node", "inca", "package"]);

    expect(process.exitCode).toBe(1);
    expect(written(stderr)).toContain("[inca] package failed: no host binary");
  });

  it.each(["SIGINT", "SIGTERM"] as const)(
    "aborts dev()'s signal on %s — a script runner may deliver either on Ctrl-C",
    (signal) => {
      void run(["node", "inca", "dev"]);

      const options = mockedDev.mock.calls[0]?.[0];
      expect(options?.signal.aborted).toBe(false);

      process.emit(signal);

      expect(options?.signal.aborted).toBe(true);
      expect(written(stderr)).toContain("[inca] shutting down");
    },
  );

  it("sets a non-zero exit code and prints a readable error when dev fails", async () => {
    mockedDev.mockRejectedValueOnce(new Error("inca dev is already running for this app (pid 42)"));

    await run(["node", "inca", "dev"]);

    expect(process.exitCode).toBe(1);
    expect(written(stderr)).toContain(
      "[inca] dev failed: inca dev is already running for this app (pid 42)",
    );
  });
});
