// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { temp, writeHostPackage } from "../../tests/support/hostPackage.mts";
import { inspectHostPackage } from "./inspect.mts";
import { hostLicenseFiles } from "./notice.mts";
import type { HostPackageState } from "./inspect.mts";
import type { HostTarget } from "./package.mts";

vi.mock("./inspect.mts", () => ({
  inspectHostPackage: vi.fn<() => Promise<HostPackageState>>(),
}));

const mockedInspect = vi.mocked(inspectHostPackage);

const TARGET: HostTarget = { key: "linux-x64", name: "@incajs/host-linux-x64" };

afterEach(() => {
  vi.unstubAllEnvs();
});

beforeEach(() => {
  mockedInspect.mockReset();
  vi.stubEnv("INCA_HOST_BIN", "");
});

describe("hostLicenseFiles", () => {
  it("lists the license files the installed package carries, in a fixed order", async () => {
    const pkg = writeHostPackage(path.join(temp(), "pkg"), Buffer.from("binary"));
    await writeFile(path.join(pkg.dir, "LICENSE-APACHE"), "a");
    await writeFile(path.join(pkg.dir, "LICENSE-MIT"), "m");
    await writeFile(path.join(pkg.dir, "NOTICE"), "n");
    mockedInspect.mockResolvedValue({ state: "installed", target: TARGET, pkg });

    expect(await hostLicenseFiles()).toEqual([
      path.join(pkg.dir, "NOTICE"),
      path.join(pkg.dir, "LICENSE-MIT"),
      path.join(pkg.dir, "LICENSE-APACHE"),
    ]);
  });

  it("skips a directory named like a license file", async () => {
    const pkg = writeHostPackage(path.join(temp(), "pkg"), Buffer.from("binary"));
    await writeFile(path.join(pkg.dir, "NOTICE"), "n");
    await mkdir(path.join(pkg.dir, "LICENSE-MIT"));
    mockedInspect.mockResolvedValue({ state: "installed", target: TARGET, pkg });

    expect(await hostLicenseFiles()).toEqual([path.join(pkg.dir, "NOTICE")]);
  });

  it("lists nothing for a host the hostBin option names", async () => {
    const pkg = writeHostPackage(path.join(temp(), "pkg"), Buffer.from("binary"));
    await writeFile(path.join(pkg.dir, "NOTICE"), "n");
    mockedInspect.mockResolvedValue({ state: "installed", target: TARGET, pkg });

    expect(await hostLicenseFiles("/my/host")).toEqual([]);
  });

  it("lists nothing for a host INCA_HOST_BIN names", async () => {
    const pkg = writeHostPackage(path.join(temp(), "pkg"), Buffer.from("binary"));
    await writeFile(path.join(pkg.dir, "NOTICE"), "n");
    mockedInspect.mockResolvedValue({ state: "installed", target: TARGET, pkg });
    vi.stubEnv("INCA_HOST_BIN", "/my/host");

    expect(await hostLicenseFiles()).toEqual([]);
  });

  it("lists nothing without an installed host package", async () => {
    mockedInspect.mockResolvedValue({ state: "notInstalled", target: TARGET });

    expect(await hostLicenseFiles()).toEqual([]);
  });
});
