// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { chmod, cp, mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { build } from "./build.mts";
import { resolveHostBin } from "./dev-client/index.mts";
import { readAppMetadata, slugify } from "./metadata.mts";
import { encodePlist } from "./plist.mts";
import type { Bundler } from "./adapter/types.mts";
import type { AppMetadata } from "./metadata.mts";
import type { PlistValue } from "./plist.mts";

/** A platform `inca package` can emit a distributable application for. */
export type PackageTarget = "macos" | "linux";

/** Options for {@link packageApp}. */
export interface PackageAppOptions {
  /** The app's root directory. Defaults to `process.cwd()`. */
  cwd?: string;
  /**
   * The app's entry point. Defaults to resolving it the same way `inca
   * dev`/`build` do: a committed `src/main.mts`, or `src/App.vue` wrapped
   * in a synthesized one.
   */
  entry?: string;
  /** Overrides the bundler — see {@link build}'s default. */
  bundler?: Bundler;
  /**
   * Overrides which `inca-host` binary gets embedded in the packaged
   * app, in place of automatic resolution (`INCA_HOST_BIN`, then the
   * per-platform `@incajs/host-*` package — see {@link resolveHostBin}).
   */
  hostBin?: string;
  /**
   * The platform to package for. Defaults to whichever one the current
   * process is running on.
   */
  target?: PackageTarget;
  /** Where progress notes go. Defaults to `process.stdout`. */
  stdout?: NodeJS.WritableStream;
}

/** The result of a successful {@link packageApp} call. */
export interface PackageResult {
  /** The packaged app's path — a `.app` directory on macOS, a plain directory on Linux. */
  appPath: string;
}

/**
 * Maps the current process's platform onto a supported packaging target.
 *
 * @throws on any platform besides macOS and Linux.
 */
function targetForPlatform(): PackageTarget {
  if (process.platform === "darwin") return "macos";
  if (process.platform === "linux") return "linux";
  throw new Error(`inca package doesn't support ${process.platform} yet`);
}

/** Inputs shared by every platform's app layout. */
interface LayoutArgs {
  distDir: string;
  metadata: AppMetadata;
  bundlePath: string;
  hostBin: string;
}

/** Writes a macOS `.app` bundle: `Contents/{MacOS,Resources}`, `Info.plist`, `PkgInfo`. */
async function packageMacos({
  distDir,
  metadata,
  bundlePath,
  hostBin,
}: LayoutArgs): Promise<string> {
  const appPath = path.join(distDir, `${metadata.productName}.app`);
  await rm(appPath, { recursive: true, force: true });

  const contentsDir = path.join(appPath, "Contents");
  const macosDir = path.join(contentsDir, "MacOS");
  const resourcesDir = path.join(contentsDir, "Resources");
  await mkdir(macosDir, { recursive: true });
  await mkdir(resourcesDir, { recursive: true });

  const executablePath = path.join(macosDir, metadata.productName);
  await cp(hostBin, executablePath);
  await chmod(executablePath, 0o755);

  await cp(bundlePath, path.join(resourcesDir, "bundle.js"));

  const plist: Record<string, PlistValue> = {
    CFBundleName: metadata.productName,
    CFBundleDisplayName: metadata.productName,
    CFBundleExecutable: metadata.productName,
    CFBundleIdentifier: metadata.identifier,
    CFBundleVersion: metadata.version,
    CFBundleShortVersionString: metadata.version,
    CFBundlePackageType: "APPL",
    CFBundleInfoDictionaryVersion: "6.0",
    NSHighResolutionCapable: true,
  };

  if (metadata.icon) {
    const iconFile = path.basename(metadata.icon);
    await cp(metadata.icon, path.join(resourcesDir, iconFile));
    plist.CFBundleIconFile = iconFile;
  }

  await writeFile(path.join(contentsDir, "Info.plist"), encodePlist(plist));
  // The classic four-byte type/creator marker every .app has carried since
  // Classic Mac OS — still expected to be there, even though nothing reads it.
  await writeFile(path.join(contentsDir, "PkgInfo"), "APPL????");

  return appPath;
}

/** Writes a plain directory holding the host executable and `bundle.js` beside it. */
async function packageLinux({
  distDir,
  metadata,
  bundlePath,
  hostBin,
}: LayoutArgs): Promise<string> {
  const dirName = slugify(metadata.productName);
  const appDir = path.join(distDir, dirName);
  await rm(appDir, { recursive: true, force: true });
  await mkdir(appDir, { recursive: true });

  const executablePath = path.join(appDir, dirName);
  await cp(hostBin, executablePath);
  await chmod(executablePath, 0o755);

  await cp(bundlePath, path.join(appDir, "bundle.js"));

  return appDir;
}

/**
 * Builds the app and pairs it with the prebuilt `inca-host` into a
 * distributable, platform-native application — a `.app` on macOS, a plain
 * directory on Linux — written alongside the build output, under the
 * app's own `dist/`. The app launches with no arguments and no terminal,
 * since `inca-host` finds its own bundle beside its executable.
 *
 * @throws if the app's metadata can't be read, the build fails, or no
 * `inca-host` binary can be resolved for the target platform.
 */
export async function packageApp(options: PackageAppOptions = {}): Promise<PackageResult> {
  const cwd = options.cwd ?? process.cwd();
  const stdout = options.stdout ?? process.stdout;
  const target = options.target ?? targetForPlatform();

  const metadata = await readAppMetadata(cwd);
  if (metadata.identifierIsDefault) {
    stdout.write(
      `[inca] no "inca.identifier" set in package.json — using generated identifier ` +
        `${metadata.identifier}\n`,
    );
  }

  const bundlePath = await build({ cwd, entry: options.entry, bundler: options.bundler });
  const distDir = path.dirname(bundlePath);

  const hostBin = options.hostBin ?? resolveHostBin();
  if (!existsSync(hostBin)) {
    throw new Error(`no host binary at ${hostBin} — check that it was built and is executable`);
  }

  const layoutArgs: LayoutArgs = { distDir, metadata, bundlePath, hostBin };
  const appPath =
    target === "macos" ? await packageMacos(layoutArgs) : await packageLinux(layoutArgs);

  return { appPath };
}
