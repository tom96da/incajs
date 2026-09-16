// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { chmod, mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { Writable } from "node:stream";

import { afterEach, describe, expect, it } from "vitest";

import { packageApp } from "./package.mts";
import type { Bundler, BuildOutput, Watcher } from "./adapter/types.mts";

let cwd: string | undefined;

afterEach(async () => {
  if (cwd) await rm(cwd, { recursive: true, force: true });
  cwd = undefined;
});

/**
 * A bundler stand-in that writes real content to `outDir`, since packaging
 * copies it onward — an entry plus a chunk, so the copy has to preserve
 * both, not just the entry.
 */
function fakeBundler(): Bundler {
  return {
    watch: (): Promise<Watcher> => Promise.reject(new Error("not used by packageApp()")),
    build: async ({ outDir }): Promise<BuildOutput> => {
      await mkdir(path.join(outDir, "chunks"), { recursive: true });
      await writeFile(path.join(outDir, "bundle.js"), "console.log('packaged');\n");
      await writeFile(path.join(outDir, "chunks", "shared.js"), "export const shared = true;\n");
      return {
        outDir,
        entryFile: path.join(outDir, "bundle.js"),
        files: ["bundle.js", "chunks/shared.js"],
      };
    },
  };
}

/** A stand-in host binary — real content and the executable bit, nothing that runs. */
async function makeHostBin(dir: string): Promise<string> {
  const hostBin = path.join(dir, "inca-host");
  await writeFile(hostBin, "#!/bin/sh\necho stand-in\n");
  await chmod(hostBin, 0o755);
  return hostBin;
}

async function makeApp(pkg: object): Promise<string> {
  cwd = await mkdtemp(path.join(tmpdir(), "inca-package-"));
  await writeFile(path.join(cwd, "package.json"), JSON.stringify(pkg));
  return cwd;
}

function makeSink(): { stream: Writable; text: () => string } {
  const chunks: string[] = [];
  const stream = new Writable({
    write(chunk, _encoding, callback) {
      chunks.push(String(chunk));
      callback();
    },
  });
  return { stream, text: () => chunks.join("") };
}

describe("packageApp", () => {
  it("lays out a macOS .app with the host, bundle and Info.plist", async () => {
    const app = await makeApp({ name: "click_counter", version: "1.2.3" });
    const hostBin = await makeHostBin(app);

    const { appPath } = await packageApp({
      cwd: app,
      entry: "unused",
      bundler: fakeBundler(),
      hostBin,
      target: "macos",
      stdout: makeSink().stream,
    });

    expect(appPath).toBe(path.join(app, "dist/click_counter.app"));

    const executable = path.join(appPath, "Contents/MacOS/click_counter");
    expect(await readFile(executable, "utf8")).toContain("stand-in");
    expect((await stat(executable)).mode & 0o111).not.toBe(0);

    expect(await readFile(path.join(appPath, "Contents/Resources/bundle.js"), "utf8")).toContain(
      "packaged",
    );
    expect(
      await readFile(path.join(appPath, "Contents/Resources/chunks/shared.js"), "utf8"),
    ).toContain("shared");
    expect(await readFile(path.join(appPath, "Contents/PkgInfo"), "utf8")).toBe("APPL????");

    const plist = await readFile(path.join(appPath, "Contents/Info.plist"), "utf8");
    expect(plist).toContain("<key>CFBundleExecutable</key>\n\t<string>click_counter</string>");
    expect(plist).toContain(
      "<key>CFBundleIdentifier</key>\n\t<string>org.inca.click-counter</string>",
    );
    expect(plist).toContain("<key>CFBundleVersion</key>\n\t<string>1.2.3</string>");
    expect(plist).toContain("<key>NSHighResolutionCapable</key>\n\t<true/>");
  });

  it("lays out a plain Linux directory with the host and bundle", async () => {
    const app = await makeApp({ name: "click_counter", version: "1.2.3" });
    const hostBin = await makeHostBin(app);

    const { appPath } = await packageApp({
      cwd: app,
      entry: "unused",
      bundler: fakeBundler(),
      hostBin,
      target: "linux",
      stdout: makeSink().stream,
    });

    expect(appPath).toBe(path.join(app, "dist/click-counter"));
    expect(await readFile(path.join(appPath, "click-counter"), "utf8")).toContain("stand-in");
    expect(await readFile(path.join(appPath, "bundle.js"), "utf8")).toContain("packaged");
    expect(await readFile(path.join(appPath, "chunks/shared.js"), "utf8")).toContain("shared");
  });

  it("reports a generated identifier when the app didn't set one", async () => {
    const app = await makeApp({ name: "click_counter", version: "1.0.0" });
    const hostBin = await makeHostBin(app);
    const sink = makeSink();

    await packageApp({
      cwd: app,
      entry: "unused",
      bundler: fakeBundler(),
      hostBin,
      target: "linux",
      stdout: sink.stream,
    });

    expect(sink.text()).toContain("org.inca.click-counter");
  });

  it("stays quiet about the identifier when the app set one", async () => {
    const app = await makeApp({
      name: "click_counter",
      version: "1.0.0",
      inca: { identifier: "com.example.click-counter" },
    });
    const hostBin = await makeHostBin(app);
    const sink = makeSink();

    await packageApp({
      cwd: app,
      entry: "unused",
      bundler: fakeBundler(),
      hostBin,
      target: "linux",
      stdout: sink.stream,
    });

    expect(sink.text()).toBe("");
  });

  it("rejects a missing host binary with a clear message", async () => {
    const app = await makeApp({ name: "click_counter", version: "1.0.0" });

    await expect(
      packageApp({
        cwd: app,
        entry: "unused",
        bundler: fakeBundler(),
        hostBin: path.join(app, "nonexistent-host"),
        target: "linux",
        stdout: makeSink().stream,
      }),
    ).rejects.toThrow(/no host binary/);
  });
});
