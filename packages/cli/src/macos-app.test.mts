// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { chmod, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { writeMacosApp } from "./macos-app.mts";

let dir: string | undefined;

afterEach(async () => {
  if (dir) await rm(dir, { recursive: true, force: true });
  dir = undefined;
});

const metadata = {
  productName: "Demo",
  identifier: "org.inca.demo",
  version: "1.2.3",
};

/** A stand-in host binary — real content and the executable bit, nothing that runs. */
async function makeHostBin(): Promise<{ dir: string; hostBin: string }> {
  dir = await mkdtemp(path.join(tmpdir(), "inca-macos-app-"));
  const hostBin = path.join(dir, "inca-host");
  await writeFile(hostBin, "#!/bin/sh\necho stand-in\n");
  await chmod(hostBin, 0o755);
  return { dir, hostBin };
}

describe("writeMacosApp", () => {
  it("names the executable and Info.plist after the app", async () => {
    const { dir: root, hostBin } = await makeHostBin();

    const app = await writeMacosApp({
      appPath: path.join(root, "Demo.app"),
      metadata,
      hostBin,
    });

    expect(app.executablePath).toBe(path.join(root, "Demo.app/Contents/MacOS/Demo"));
    expect(app.resourcesDir).toBe(path.join(root, "Demo.app/Contents/Resources"));
    expect((await stat(app.executablePath)).mode & 0o111).toBeTruthy();

    const plist = await readFile(path.join(root, "Demo.app/Contents/Info.plist"), "utf8");
    expect(plist).toContain("<string>Demo</string>");
    expect(plist).toContain("<string>org.inca.demo</string>");
    expect(plist).toContain("<string>1.2.3</string>");
    expect(plist).not.toContain("CFBundleIconFile");
  });

  it("shares one inode with the host binary when asked to link", async () => {
    const { dir: root, hostBin } = await makeHostBin();

    const app = await writeMacosApp({
      appPath: path.join(root, "Demo.app"),
      metadata,
      hostBin,
      link: true,
    });

    expect((await stat(app.executablePath)).ino).toBe((await stat(hostBin)).ino);
  });

  it("copies an icon into Resources and names it in Info.plist", async () => {
    const { dir: root, hostBin } = await makeHostBin();
    const icon = path.join(root, "app.icns");
    await writeFile(icon, "icon");

    await writeMacosApp({
      appPath: path.join(root, "Demo.app"),
      metadata: { ...metadata, icon },
      hostBin,
    });

    expect(await readFile(path.join(root, "Demo.app/Contents/Resources/app.icns"), "utf8")).toBe(
      "icon",
    );
    const plist = await readFile(path.join(root, "Demo.app/Contents/Info.plist"), "utf8");
    expect(plist).toContain("<key>CFBundleIconFile</key>");
    expect(plist).toContain("<string>app.icns</string>");
  });

  it("replaces a bundle already at that path", async () => {
    const { dir: root, hostBin } = await makeHostBin();
    const appPath = path.join(root, "Demo.app");

    await writeMacosApp({ appPath, metadata, hostBin });
    await writeFile(path.join(appPath, "Contents/Resources/stale.js"), "stale");
    await writeMacosApp({ appPath, metadata, hostBin });

    await expect(stat(path.join(appPath, "Contents/Resources/stale.js"))).rejects.toThrow("ENOENT");
  });
});
