// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { PassThrough } from "node:stream";

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { pretendPlatform } from "../../tests/support/dev.mts";
import { regularFile, temp, writeHostPackage } from "../../tests/support/hostPackage.mts";
import { resolveHostBin } from "./bin.mts";
import { locateHostPackageDir } from "./package.mts";
import type { IncaError } from "../error.mts";

vi.mock("./package.mts", async (importActual) => ({
  ...(await importActual<typeof import("./package.mts")>()),
  locateHostPackageDir: vi.fn<(name: string) => string | undefined>(),
}));
vi.mock("node:os", async (importOriginal) => {
  const actual = await importOriginal<typeof import("node:os")>();
  return { ...actual, homedir: vi.fn<() => string>(actual.homedir) };
});

const ENV_VAR = "INCA_HOST_BIN";
const mockedLocate = vi.mocked(locateHostPackageDir);
const mockedHomedir = vi.mocked(homedir);

let cacheDir: string;
let home: string;
let binary: Buffer;

/** An app directory that has a `node_modules`. */
function appWithModules(): string {
  const cwd = temp();
  mkdirSync(path.join(cwd, "node_modules"));
  return cwd;
}

/** Makes the host package lookup find a package that carries the fake binary. */
function installPackage(): void {
  mockedLocate.mockReturnValue(writeHostPackage(path.join(temp(), "pkg"), binary).dir);
}

/** `resolveHostBin` with its unpack line sent to `stderr`. */
function resolve(cwd?: string, stderr: NodeJS.WritableStream = new PassThrough()): Promise<string> {
  return resolveHostBin(cwd, stderr);
}

/** The error `resolveHostBin` throws. */
async function thrown(cwd?: string): Promise<IncaError> {
  try {
    await resolve(cwd);
  } catch (error) {
    return error as IncaError;
  }
  throw new Error("did not throw");
}

beforeEach(() => {
  binary = Buffer.from(`#!/bin/sh\nexit 0\n${Math.random()}`);
  cacheDir = temp();
  home = temp();
  vi.stubEnv("INCA_CACHE_DIR", cacheDir);
  mockedHomedir.mockReturnValue(home);
  vi.stubEnv("XDG_CACHE_HOME", "");
  mockedLocate.mockReset();
  mockedLocate.mockReturnValue(undefined);
  mockedHomedir.mockClear();
});

afterEach(() => {
  vi.unstubAllEnvs();
  delete process.env[ENV_VAR];
});

describe("resolveHostBin", () => {
  it("returns INCA_HOST_BIN unchanged when it's set", async () => {
    process.env[ENV_VAR] = "/custom/path/to/inca-host";

    expect(await resolve()).toBe("/custom/path/to/inca-host");
  });

  it("skips the package and the cache when INCA_HOST_BIN is set", async () => {
    process.env[ENV_VAR] = "/custom/path/to/inca-host";
    installPackage();
    mockedLocate.mockClear();

    await resolve();

    expect(mockedLocate).not.toHaveBeenCalled();
    expect(readdirSync(cacheDir)).toEqual([]);
  });

  it("unpacks the host package of this platform into the cache", async () => {
    installPackage();

    const bin = await resolve();

    expect(bin).toMatch(new RegExp(`^${cacheDir}/(linux|darwin)-(x64|arm64)@1\\.2\\.3/inca-host$`));
    expect(readFileSync(bin)).toEqual(binary);
  });

  it("writes the unpack line to the stream it is given", async () => {
    installPackage();
    const stderr = new PassThrough();

    const bin = await resolve(undefined, stderr);

    expect(String(stderr.read())).toContain(`unpacked the host binary into ${path.dirname(bin)}`);
  });

  it("falls back to the user cache when INCA_CACHE_DIR can't be used", async () => {
    installPackage();
    vi.stubEnv("INCA_CACHE_DIR", regularFile());

    const bin = await resolve();

    const userCache =
      process.platform === "darwin"
        ? path.join(home, "Library/Caches/incajs/host")
        : path.join(home, ".cache/incajs/host");
    expect(bin.startsWith(`${userCache}${path.sep}`)).toBe(true);
  });

  it("falls back to node_modules of the app when the user cache can't be used", async () => {
    installPackage();
    vi.stubEnv("INCA_CACHE_DIR", regularFile());
    mockedHomedir.mockReturnValue(regularFile());
    const cwd = appWithModules();

    expect((await resolve(cwd)).startsWith(path.join(cwd, "node_modules/.inca/host"))).toBe(true);
  });

  it("treats a failing homedir as no home directory", async () => {
    installPackage();
    vi.stubEnv("INCA_CACHE_DIR", regularFile());
    mockedHomedir.mockImplementationOnce(() => {
      throw new Error("no home");
    });
    const cwd = appWithModules();

    expect((await resolve(cwd)).startsWith(path.join(cwd, "node_modules/.inca/host"))).toBe(true);
  });

  it("treats an empty HOME as no home directory", async () => {
    installPackage();
    vi.stubEnv("INCA_CACHE_DIR", regularFile());
    mockedHomedir.mockReturnValueOnce("");
    const cwd = appWithModules();

    expect((await resolve(cwd)).startsWith(path.join(cwd, "node_modules/.inca/host"))).toBe(true);
  });

  it("reports no binary when the package carries none", async () => {
    const dir = temp();
    writeFileSync(path.join(dir, "package.json"), "{}");
    mockedLocate.mockReturnValue(dir);

    const error = await thrown();

    expect(error.code).toBe("ERR_INCA_HOST_BIN_UNRESOLVED");
    expect(error.message).toContain("is installed but carries no host binary");
  });

  it("reports no binary when the package isn't installed", async () => {
    const error = await thrown();

    expect(error.code).toBe("ERR_INCA_HOST_BIN_UNRESOLVED");
    expect(error.message).toContain("@incajs/host-");
    expect(error.message).toContain("isn't installed");
  });

  it("reports a damaged package", async () => {
    const dir = temp();
    writeFileSync(
      path.join(dir, "package.json"),
      JSON.stringify({ version: "1.2.3", incajs: { host: { sha256: "x" } } }),
    );
    mockedLocate.mockReturnValue(dir);

    expect((await thrown()).code).toBe("ERR_INCA_HOST_PACKAGE_CORRUPT");
  });

  it("reports a platform with no published package", async () => {
    pretendPlatform("win32");

    const error = await thrown();

    expect(error.code).toBe("ERR_INCA_HOST_BIN_UNRESOLVED");
    expect(error.message).toContain("no host package is published for win32/");
  });
});
