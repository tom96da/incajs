// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import fs from "node:fs";
import path from "node:path";

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
vi.mock("./info.mts", () => ({
  info: vi.fn<(stdout?: NodeJS.WritableStream) => Promise<void>>(),
}));
vi.mock("./printConfig.mts", () => ({
  printConfig: vi.fn<(options?: PrintConfigOptions) => Promise<number>>(),
  checkBuiltConfig: vi.fn<(entry: string, stderr: NodeJS.WritableStream) => Promise<void>>(),
}));

import { build } from "./build.mts";
import { run } from "./cli.mts";
import { dev } from "./dev.mts";
import { IncaError } from "./error.mts";
import { info } from "./info.mts";
import { packageApp } from "./package.mts";
import { checkBuiltConfig, printConfig } from "./printConfig.mts";
import type { BuildAppOptions } from "./build.mts";
import type { DevOptions } from "./dev.mts";
import type { PackageAppOptions, PackageResult } from "./package.mts";
import type { PrintConfigOptions } from "./printConfig.mts";
import type { BuildOutput } from "./types.mts";

const mockedDev = vi.mocked(dev);
const mockedBuild = vi.mocked(build);
const mockedPackageApp = vi.mocked(packageApp);
const mockedPrintConfig = vi.mocked(printConfig);
const mockedInfo = vi.mocked(info);
const mockedCheckBuiltConfig = vi.mocked(checkBuiltConfig);

let stdout: MockInstance;
let stderr: MockInstance;
let consoleLog: MockInstance;
let consoleError: MockInstance;
let exit: MockInstance;

function written(spy: MockInstance): string {
  return spy.mock.calls.map((call) => String(call[0])).join("");
}

/** What citty printed through `console.log` and `console.error`. */
function printed(): string {
  return [consoleLog, consoleError].map(written).join("");
}

beforeEach(() => {
  // citty ends a help or usage run with `process.exit`.
  exit = vi.spyOn(process, "exit").mockImplementation((() => {}) as () => never);
  consoleLog = vi.spyOn(console, "log").mockImplementation(() => {});
  consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  stdout = vi.spyOn(process.stdout, "write").mockImplementation(() => true);
  stderr = vi.spyOn(process.stderr, "write").mockImplementation(() => true);
});

afterEach(() => {
  exit.mockRestore();
  consoleLog.mockRestore();
  consoleError.mockRestore();
  stdout.mockRestore();
  stderr.mockRestore();
  mockedDev.mockClear();
  mockedBuild.mockClear();
  mockedPackageApp.mockClear();
  mockedPrintConfig.mockClear();
  mockedCheckBuiltConfig.mockReset();
  mockedInfo.mockReset();
  process.removeAllListeners("SIGINT");
  process.removeAllListeners("SIGTERM");
  process.exitCode = undefined;
  // Bun ignores `undefined` here.
  if (process.exitCode !== undefined) process.exitCode = 0;
  vi.unstubAllEnvs();
});

describe("run", () => {
  it.each([
    [[], "No command specified."],
    [["bogus"], "Unknown command"],
  ] as const)("prints usage and exits with 1 for %j", async (args, message) => {
    await run(["node", "inca", ...args]);

    expect(exit).toHaveBeenCalledWith(1);
    expect(printed()).toContain("USAGE");
    expect(printed()).toContain(message);
    expect(mockedDev).not.toHaveBeenCalled();
    expect(mockedBuild).not.toHaveBeenCalled();
    expect(mockedPackageApp).not.toHaveBeenCalled();
  });

  it("prints the commands and exits with 0 for --help", async () => {
    await run(["node", "inca", "--help"]);

    expect(exit).toHaveBeenCalledWith(0);
    expect(printed()).toMatch(/dev[\s\S]*build[\s\S]*package/);
  });

  it.each(["dev", "build", "package", "config", "info"])(
    "prints the usage of %s",
    async (command) => {
      await run(["node", "inca", command, "--help"]);

      expect(exit).toHaveBeenCalledWith(0);
      expect(printed()).toContain(`inca ${command}`);
      expect(mockedDev).not.toHaveBeenCalled();
      expect(mockedBuild).not.toHaveBeenCalled();
      expect(mockedPackageApp).not.toHaveBeenCalled();
      expect(mockedPrintConfig).not.toHaveBeenCalled();
      expect(mockedInfo).not.toHaveBeenCalled();
    },
  );

  it("prints the package version for --version", async () => {
    await run(["node", "inca", "--version"]);

    expect(printed().trim()).toMatch(/^\d+\.\d+\.\d+/);
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
    expect(process.exitCode ?? 0).toBe(0);
    expect(written(stdout)).toContain(
      `[inca] built ${path.relative(process.cwd(), "/app/dist/bundle.js")}`,
    );
  });

  it("sets a non-zero exit code and prints a readable error when build fails", async () => {
    mockedBuild.mockRejectedValue(new Error("syntax error"));

    await run(["node", "inca", "build"]);

    expect(process.exitCode).toBe(1);
    expect(written(stderr)).toContain("[inca] build failed: syntax error");
  });

  it("checks the config of the built entry", async () => {
    mockedBuild.mockResolvedValue({
      outDir: "/app/dist",
      entryFile: "/app/dist/bundle.js",
      files: ["bundle.js"],
    });

    await run(["node", "inca", "build"]);

    expect(mockedCheckBuiltConfig).toHaveBeenCalledWith("/app/dist/bundle.js", process.stderr);
  });

  it("fails build when the config check throws", async () => {
    mockedBuild.mockResolvedValue({
      outDir: "/app/dist",
      entryFile: "/app/dist/bundle.js",
      files: ["bundle.js"],
    });
    mockedCheckBuiltConfig.mockRejectedValue(
      new IncaError("ERR_INCA_CONFIG_INVALID", "window.width in inca.config.ts is -100"),
    );

    await run(["node", "inca", "build"]);

    expect(process.exitCode).toBe(1);
    expect(written(stderr)).toContain("build failed (ERR_INCA_CONFIG_INVALID)");
  });

  it("runs package and leaves the exit code untouched on success", async () => {
    mockedPackageApp.mockResolvedValue({ appPath: "/app/dist/click_counter.app" });

    await run(["node", "inca", "package"]);

    expect(mockedPackageApp).toHaveBeenCalled();
    expect(process.exitCode ?? 0).toBe(0);
    expect(written(stdout)).toContain(
      `[inca] packaged ${path.relative(process.cwd(), "/app/dist/click_counter.app")}`,
    );
  });

  it("sets a non-zero exit code and prints a readable error when package fails", async () => {
    mockedPackageApp.mockRejectedValue(new Error("no host binary"));

    await run(["node", "inca", "package"]);

    expect(process.exitCode).toBe(1);
    expect(written(stderr)).toContain("[inca] package failed: no host binary");
  });

  it("sets the exit code to the one config returns", async () => {
    mockedPrintConfig.mockResolvedValue(3);

    await run(["node", "inca", "config"]);

    expect(mockedPrintConfig).toHaveBeenCalled();
    expect(process.exitCode).toBe(3);
  });

  it("sets a non-zero exit code and prints a readable error when config fails", async () => {
    mockedPrintConfig.mockRejectedValue(new Error("no host binary"));

    await run(["node", "inca", "config"]);

    expect(process.exitCode).toBe(1);
    expect(written(stderr)).toContain("[inca] config failed: no host binary");
  });

  it("runs info and leaves the exit code untouched on success", async () => {
    await run(["node", "inca", "info"]);

    expect(mockedInfo).toHaveBeenCalledOnce();
    expect(process.exitCode ?? 0).toBe(0);
  });

  it("sets a non-zero exit code and prints a readable error when info fails", async () => {
    mockedInfo.mockRejectedValue(new Error("m"));

    await run(["node", "inca", "info"]);

    expect(process.exitCode).toBe(1);
    expect(written(stderr)).toContain("[inca] info failed: m");
  });

  it.each(["SIGINT", "SIGTERM"] as const)(
    "aborts dev()'s signal on %s: a script runner may deliver either on Ctrl-C",
    async (signal) => {
      void run(["node", "inca", "dev"]);
      await vi.waitFor(() => expect(mockedDev).toHaveBeenCalled());

      const options = mockedDev.mock.calls[0]?.[0];
      expect(options?.signal.aborted).toBe(false);

      process.emit(signal);

      expect(options?.signal.aborted).toBe(true);
      expect(written(stderr)).toContain("[inca] shutting down");
    },
  );

  it("leaves fds 0-2 open when dev() returns normally", async () => {
    const close = vi.spyOn(fs, "closeSync").mockImplementation(() => {});
    try {
      mockedDev.mockResolvedValueOnce(undefined);
      await run(["node", "inca", "dev"]);
      expect(close).not.toHaveBeenCalled();
    } finally {
      close.mockRestore();
    }
  });

  it("closes fds 0-2 once dev() returns after a signal", async () => {
    const close = vi.spyOn(fs, "closeSync").mockImplementation(() => {});
    try {
      mockedDev.mockImplementationOnce(async ({ signal }) => {
        process.emit("SIGINT");
        expect(signal.aborted).toBe(true);
      });
      await run(["node", "inca", "dev"]);
      expect(close.mock.calls.map((call) => call[0])).toEqual([0, 1, 2]);
    } finally {
      close.mockRestore();
    }
  });

  it("shuts down once when both signals arrive", async () => {
    void run(["node", "inca", "dev"]);
    await vi.waitFor(() => expect(mockedDev).toHaveBeenCalled());

    process.emit("SIGINT");
    process.emit("SIGTERM");

    expect(written(stderr).match(/\[inca\] shutting down/g)).toHaveLength(1);
  });

  it("sets a non-zero exit code and prints a readable error when dev fails", async () => {
    mockedDev.mockRejectedValueOnce(new Error("inca dev is already running for this app (pid 42)"));

    await run(["node", "inca", "dev"]);

    expect(process.exitCode).toBe(1);
    expect(written(stderr)).toContain(
      "[inca] dev failed: inca dev is already running for this app (pid 42)",
    );
  });

  it.each([
    [["--sourcemap"], true],
    [["--no-sourcemap"], false],
    [["--sourcemap", "--no-sourcemap"], false],
    [["--no-sourcemap", "--sourcemap"], false],
    [[], undefined],
  ] as const)("passes sourcemap %j to build as %j", async (flags, expected) => {
    mockedBuild.mockResolvedValue({
      outDir: "/app/dist",
      entryFile: "/app/dist/bundle.js",
      files: ["bundle.js"],
    });

    await run(["node", "inca", "build", ...flags]);

    expect(mockedBuild.mock.calls[0]?.[0]?.sourcemap).toBe(expected);
  });

  it.each([
    [["--sourcemap"], true],
    [["--no-sourcemap"], false],
  ] as const)("passes sourcemap %j to package as %j", async (flags, expected) => {
    mockedPackageApp.mockResolvedValue({ appPath: "/app/dist/click_counter.app" });

    await run(["node", "inca", "package", ...flags]);

    expect(mockedPackageApp.mock.calls[0]?.[0]?.sourcemap).toBe(expected);
  });

  it.each(["--sourcemap", "--no-sourcemap"])("warns that dev ignores %s", async (flag) => {
    mockedDev.mockResolvedValueOnce(undefined);

    await run(["node", "inca", "dev", flag]);

    expect(written(stderr)).toMatch(/\[inca\] dev ignores --sourcemap and --no-sourcemap\n/);
    expect(mockedDev.mock.calls[0]?.[0]).not.toHaveProperty("sourcemap");
    expect(process.exitCode ?? 0).toBe(0);
  });

  it("stays silent about the source map flags for dev when none is given", async () => {
    mockedDev.mockResolvedValueOnce(undefined);

    await run(["node", "inca", "dev"]);

    expect(written(stderr)).not.toContain("ignores");
  });

  it("leaves experimentalHmr off and stays silent about it by default", async () => {
    mockedDev.mockResolvedValueOnce(undefined);

    await run(["node", "inca", "dev"]);

    expect(mockedDev.mock.calls[0]?.[0].experimentalHmr).toBe(false);
    expect(written(stdout)).not.toContain("HMR");
  });

  it("turns experimentalHmr on with --experimental-hmr and announces it", async () => {
    mockedDev.mockResolvedValueOnce(undefined);

    await run(["node", "inca", "dev", "--experimental-hmr"]);

    expect(mockedDev.mock.calls[0]?.[0].experimentalHmr).toBe(true);
    // The clock follows the runner's locale, so only the shape is asserted.
    expect(written(stdout)).toMatch(/^\d.* \[inca\] experimental HMR active\n/);
  });

  it("turns experimentalHmr on with INCA_EXPERIMENTAL_HMR too", async () => {
    mockedDev.mockResolvedValueOnce(undefined);
    vi.stubEnv("INCA_EXPERIMENTAL_HMR", "1");

    await run(["node", "inca", "dev"]);

    expect(mockedDev.mock.calls[0]?.[0].experimentalHmr).toBe(true);
  });

  it.each(["0", "false"])(
    "leaves experimentalHmr off when INCA_EXPERIMENTAL_HMR=%s",
    async (value) => {
      mockedDev.mockResolvedValueOnce(undefined);
      vi.stubEnv("INCA_EXPERIMENTAL_HMR", value);

      await run(["node", "inca", "dev"]);

      expect(mockedDev.mock.calls[0]?.[0].experimentalHmr).toBe(false);
    },
  );
});
