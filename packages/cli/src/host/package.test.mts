// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { writeFileSync } from "node:fs";
import path from "node:path";

import { beforeEach, describe, expect, it } from "vitest";

import { temp, writeHostPackage } from "../../tests/support/hostPackage.mts";
import { IncaError } from "../error.mts";
import { hostTarget, locateHostPackageDir, readHostPackage } from "./package.mts";

const NAME = "@incajs/host-linux-x64";
const HASH = "a".repeat(64);

let dir: string;

beforeEach(() => {
  dir = temp();
});

/** Writes `packageJson` as the package.json of the temporary package. */
function writePackageJson(packageJson: unknown): void {
  writeFileSync(path.join(dir, "package.json"), JSON.stringify(packageJson));
}

/** The error `readHostPackage` throws for the temporary package. */
async function readError(): Promise<IncaError> {
  try {
    await readHostPackage(NAME, dir);
  } catch (error) {
    return error as IncaError;
  }
  throw new Error("readHostPackage did not throw");
}

describe("hostTarget", () => {
  it("names the target and the package of a published platform", () => {
    expect(hostTarget("darwin", "arm64")).toEqual({
      key: "darwin-arm64",
      name: "@incajs/host-darwin-arm64",
    });
  });

  it.each([
    ["win32", "x64"],
    ["linux", "ia32"],
  ])("is undefined off the published platforms (%s/%s)", (platform, arch) => {
    expect(hostTarget(platform, arch)).toBeUndefined();
  });
});

describe("locateHostPackageDir", () => {
  it("returns the directory of an installed package", () => {
    expect(path.basename(locateHostPackageDir("vitest") ?? "")).toBe("vitest");
  });

  it("returns undefined for a package that isn't installed", () => {
    expect(locateHostPackageDir("@incajs/host-nonexistent")).toBeUndefined();
  });
});

describe("readHostPackage", () => {
  it("reads the hash and version of a package", async () => {
    const written = writeHostPackage(dir, Buffer.from("binary"));

    expect(await readHostPackage(written.name, dir)).toEqual(written);
  });

  it.each([
    ["has no incajs.host", { name: "x" }],
    ["has an incajs without host", { incajs: {} }],
    ["is null", null],
  ])("returns undefined when package.json %s", async (_row, packageJson) => {
    writePackageJson(packageJson);

    expect(await readHostPackage(NAME, dir)).toBeUndefined();
  });

  it.each([
    ["incajs.host is null", { version: "1.2.3", incajs: { host: null } }],
    [
      "sha256 has uppercase digits",
      { version: "1.2.3", incajs: { host: { sha256: "A".repeat(64) } } },
    ],
    ["sha256 has 63 digits", { version: "1.2.3", incajs: { host: { sha256: "a".repeat(63) } } }],
    [
      "sha256 has a non-hex digit",
      { version: "1.2.3", incajs: { host: { sha256: `g${"a".repeat(63)}` } } },
    ],
  ])("throws for a damaged incajs.host (%s)", async (_row, packageJson) => {
    writePackageJson(packageJson);

    const error = await readError();

    expect(error.code).toBe("ERR_INCA_HOST_PACKAGE_CORRUPT");
    expect(error.message).toContain(NAME);
  });

  it.each([
    ["is missing", {}],
    ["holds a path separator", { version: "1.2.3-../x" }],
    ["has a trailing character", { version: "1.2.3 " }],
  ])("throws when the version %s", async (_row, extra) => {
    writePackageJson({ ...extra, incajs: { host: { sha256: HASH } } });

    const error = await readError();

    expect(error.code).toBe("ERR_INCA_HOST_PACKAGE_CORRUPT");
    expect(error.message).toContain("invalid version");
  });

  it.each(["1.2.3-beta.1", "1.2.3+build.5"])("accepts the version %s", async (version) => {
    writePackageJson({ version, incajs: { host: { sha256: HASH } } });

    expect((await readHostPackage(NAME, dir))?.version).toBe(version);
  });

  it("throws when package.json is missing", async () => {
    const error = await readError();

    expect(error.code).toBe("ERR_INCA_HOST_PACKAGE_CORRUPT");
    expect(error.message).toContain("package.json can't be read");
  });

  it("throws when package.json is not JSON", async () => {
    writeFileSync(path.join(dir, "package.json"), "{");

    const error = await readError();

    expect(error.code).toBe("ERR_INCA_HOST_PACKAGE_CORRUPT");
    expect(error.message).toContain("package.json can't be read");
  });
});
