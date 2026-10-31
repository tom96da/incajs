// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { writeFileSync } from "node:fs";
import path from "node:path";

import { beforeEach, describe, expect, it, vi } from "vitest";

import { temp, writeHostPackage } from "../../tests/support/hostPackage.mts";
import { inspectHostPackage } from "./inspect.mts";
import { hostTarget, locateHostPackageDir } from "./package.mts";
import type { HostTarget } from "./package.mts";

vi.mock("./package.mts", async (importActual) => ({
  ...(await importActual<typeof import("./package.mts")>()),
  locateHostPackageDir: vi.fn<(name: string) => string | undefined>(),
  hostTarget: vi.fn<() => HostTarget | undefined>(),
}));

const mockedLocate = vi.mocked(locateHostPackageDir);
const mockedHostTarget = vi.mocked(hostTarget);

const TARGET: HostTarget = { key: "linux-x64", name: "@incajs/host-linux-x64" };

beforeEach(() => {
  mockedHostTarget.mockReset();
  mockedHostTarget.mockReturnValue(TARGET);
  mockedLocate.mockReset();
  mockedLocate.mockReturnValue(undefined);
});

describe("inspectHostPackage", () => {
  it("reports a platform that has no published package", async () => {
    mockedHostTarget.mockReturnValue(undefined);

    expect(await inspectHostPackage()).toEqual({ state: "unsupported" });
  });

  it("reports a package that isn't installed", async () => {
    expect(await inspectHostPackage()).toEqual({ state: "notInstalled", target: TARGET });
  });

  it("reports an installed package that carries no binary", async () => {
    const dir = temp();
    writeFileSync(path.join(dir, "package.json"), "{}");
    mockedLocate.mockReturnValue(dir);

    expect(await inspectHostPackage()).toEqual({ state: "noBinary", target: TARGET });
  });

  it("reports a damaged package with its target", async () => {
    const dir = temp();
    writeFileSync(
      path.join(dir, "package.json"),
      JSON.stringify({ version: "1.2.3", incajs: { host: { sha256: "x" } } }),
    );
    mockedLocate.mockReturnValue(dir);

    expect(await inspectHostPackage()).toMatchObject({
      state: "corrupt",
      target: TARGET,
      error: { code: "ERR_INCA_HOST_PACKAGE_CORRUPT" },
    });
  });

  it("reports an installed package with its binary", async () => {
    const pkg = writeHostPackage(path.join(temp(), "pkg"), Buffer.from("binary"));
    mockedLocate.mockReturnValue(pkg.dir);

    expect(await inspectHostPackage()).toEqual({ state: "installed", target: TARGET, pkg });
  });
});
