// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { writeFileSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { PassThrough } from "node:stream";

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { version } from "../package.json" with { type: "json" };
import { regularFile, temp, writeHostPackage } from "../tests/support/hostPackage.mts";
import { hostTarget, locateHostPackageDir } from "./host/package.mts";
import { info } from "./info.mts";
import type { HostTarget } from "./host/package.mts";

vi.mock("./host/package.mts", async (importActual) => ({
  ...(await importActual<typeof import("./host/package.mts")>()),
  locateHostPackageDir: vi.fn<(name: string) => string | undefined>(),
  hostTarget: vi.fn<() => HostTarget | undefined>(),
}));

vi.mock("node:os", async (importOriginal) => {
  const actual = await importOriginal<typeof import("node:os")>();
  return { ...actual, homedir: vi.fn<() => string>(actual.homedir) };
});

const mockedLocate = vi.mocked(locateHostPackageDir);
const mockedHomedir = vi.mocked(homedir);
const mockedHostTarget = vi.mocked(hostTarget);

/** The lines `info` prints. */
async function output(
  cwd?: string,
  stderr: NodeJS.WritableStream = new PassThrough(),
): Promise<string[]> {
  let text = "";
  await info(
    { write: (chunk: string) => ((text += chunk), true) } as NodeJS.WritableStream,
    cwd,
    stderr,
  );
  return text.split("\n");
}

/** Makes the host package lookup find a package that carries a fake binary. */
function installPackage(): void {
  mockedLocate.mockReturnValue(writeHostPackage(path.join(temp(), "pkg"), Buffer.from("bin")).dir);
}

beforeEach(() => {
  vi.stubEnv("INCA_HOST_BIN", "");
  vi.stubEnv("INCA_CACHE_DIR", temp());
  mockedHomedir.mockReturnValue(temp());
  vi.stubEnv("XDG_CACHE_HOME", "");
  mockedLocate.mockReset();
  mockedLocate.mockReturnValue(undefined);
  mockedHostTarget.mockReset();
  mockedHostTarget.mockReturnValue({ key: "linux-x64", name: "@incajs/host-linux-x64" });
});

afterEach(() => {
  vi.unstubAllEnvs();
});

describe("info", () => {
  it("prints exactly three lines with the values in one column", async () => {
    const bin = path.join(temp(), "inca-host");
    writeFileSync(bin, "x");
    vi.stubEnv("INCA_HOST_BIN", bin);
    installPackage();

    expect(await output()).toEqual([
      `Incarnative.js: ${version}`,
      "host package:   @incajs/host-linux-x64@1.2.3",
      `host binary:    ${bin}`,
      "",
    ]);
  });

  it("prints the unpacked path of the host binary", async () => {
    const cache = temp();
    vi.stubEnv("INCA_CACHE_DIR", cache);
    installPackage();

    expect((await output())[2]).toBe(`host binary:    ${cache}/linux-x64@1.2.3/inca-host`);
  });

  it("writes the unpack line to the stream it is given", async () => {
    installPackage();
    const stderr = new PassThrough();

    await output(undefined, stderr);

    expect(String(stderr.read())).toContain("unpacked the host binary into");
  });

  it("says the platform has no host package", async () => {
    mockedHostTarget.mockReturnValue(undefined);

    expect((await output())[1]).toBe("host package:   unavailable (unsupported platform)");
  });

  it("says the host package isn't installed", async () => {
    expect((await output())[1]).toBe("host package:   @incajs/host-linux-x64 (not installed)");
  });

  it("says the host package has no binary", async () => {
    const dir = temp();
    writeFileSync(path.join(dir, "package.json"), "{}");
    mockedLocate.mockReturnValue(dir);

    expect((await output())[1]).toBe("host package:   @incajs/host-linux-x64 (no binary)");
  });

  it("reports an unresolved host binary on its line", async () => {
    expect((await output())[2]).toMatch(/^host binary: {4}unavailable \(bin unresolved\)$/);
  });

  it("reports a failed unpack on its line", async () => {
    installPackage();
    vi.stubEnv("INCA_CACHE_DIR", regularFile());
    mockedHomedir.mockReturnValue(regularFile());

    expect((await output(temp()))[2]).toMatch(/^host binary: {4}unavailable \(unpack failed\)$/);
  });

  it("reports a damaged package on the package line", async () => {
    const dir = temp();
    writeFileSync(
      path.join(dir, "package.json"),
      JSON.stringify({ incajs: { host: { sha256: "x" } } }),
    );
    mockedLocate.mockReturnValue(dir);

    expect((await output())[1]).toBe("host package:   @incajs/host-linux-x64 (corrupt)");
  });

  it("reports a missing override file on its line", async () => {
    vi.stubEnv("INCA_HOST_BIN", "/nonexistent/inca-host");

    expect((await output())[2]).toMatch(/^host binary: {4}unavailable \(bin not found\)$/);
  });

  it("rethrows an error that isn't an IncaError", async () => {
    mockedLocate.mockImplementation(() => {
      throw new TypeError("x");
    });
    const write = vi.fn<(chunk: string) => boolean>();

    await expect(info({ write } as unknown as NodeJS.WritableStream)).rejects.toThrow(TypeError);
    expect(write).not.toHaveBeenCalled();
  });
});
