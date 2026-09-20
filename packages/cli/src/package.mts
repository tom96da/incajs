// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { chmod, cp, mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { build } from "./build.mts";
import { slugify } from "./config/defaults.mts";
import { resolveAppConfig } from "./config/loader.mts";
import { resolveHostBin } from "./dev-client/index.mts";
import { IncaError } from "./error.mts";
import { log } from "./log.mts";
import { encodePlist } from "./plist.mts";
import type { Bundler, BuildOutput } from "./adapter/types.mts";
import type { ResolvedAppConfig } from "./config/loader.mts";
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
  /** Where progress notes and the bundler's own output go. Defaults to `process.stdout`. */
  stdout?: NodeJS.WritableStream;
  /** Where the bundler's warnings and errors go. Defaults to `process.stderr`. */
  stderr?: NodeJS.WritableStream;
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
  throw new IncaError(
    "ERR_INCA_PLATFORM_UNSUPPORTED",
    `inca package doesn't support ${process.platform} yet`,
  );
}

/** Inputs shared by every platform's app layout. */
interface LayoutArgs {
  metadata: ResolvedAppConfig;
  output: BuildOutput;
  hostBin: string;
}

/**
 * Copies exactly the files `output` reports — never `output.outDir`
 * wholesale, which could otherwise recurse into a previously packaged app
 * sitting alongside it — into `destDir`, preserving each file's own
 * subpath (e.g. a chunk under `chunks/`).
 */
async function copyBuildOutput(output: BuildOutput, destDir: string): Promise<void> {
  await Promise.all(
    output.files.map(async (relPath) => {
      const dest = path.join(destDir, relPath);
      await mkdir(path.dirname(dest), { recursive: true });
      await cp(path.join(output.outDir, relPath), dest);
    }),
  );
}

/** Writes a macOS `.app` bundle: `Contents/{MacOS,Resources}`, `Info.plist`, `PkgInfo`. */
async function packageMacos({ metadata, output, hostBin }: LayoutArgs): Promise<string> {
  const appPath = path.join(output.outDir, `${metadata.productName}.app`);
  await rm(appPath, { recursive: true, force: true });

  const contentsDir = path.join(appPath, "Contents");
  const macosDir = path.join(contentsDir, "MacOS");
  const resourcesDir = path.join(contentsDir, "Resources");
  await mkdir(macosDir, { recursive: true });
  await mkdir(resourcesDir, { recursive: true });

  const executablePath = path.join(macosDir, metadata.productName);
  await cp(hostBin, executablePath);
  await chmod(executablePath, 0o755);

  await copyBuildOutput(output, resourcesDir);

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
async function packageLinux({ metadata, output, hostBin }: LayoutArgs): Promise<string> {
  const dirName = slugify(metadata.productName);
  const appDir = path.join(output.outDir, dirName);
  await rm(appDir, { recursive: true, force: true });
  await mkdir(appDir, { recursive: true });

  const executablePath = path.join(appDir, dirName);
  await cp(hostBin, executablePath);
  await chmod(executablePath, 0o755);

  await copyBuildOutput(output, appDir);

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

  const metadata = await resolveAppConfig(cwd);
  if (metadata.usedPackageJsonKey) {
    log(
      stdout,
      `the "inca" key in package.json is deprecated — move these settings to inca.config.ts`,
    );
  }
  if (metadata.identifierIsDefault) {
    log(
      stdout,
      `no "identifier" set in inca.config.ts — using generated identifier ${metadata.identifier}`,
    );
  }

  const output = await build({
    cwd,
    entry: options.entry,
    config: metadata,
    bundler: options.bundler,
    stdout,
    stderr: options.stderr,
  });

  const hostBin = options.hostBin ?? resolveHostBin();
  if (!existsSync(hostBin)) {
    throw new IncaError(
      "ERR_INCA_HOST_BIN_NOT_FOUND",
      `no host binary at ${hostBin} — check that it was built and is executable`,
    );
  }

  const layoutArgs: LayoutArgs = { metadata, output, hostBin };
  const appPath =
    target === "macos" ? await packageMacos(layoutArgs) : await packageLinux(layoutArgs);

  return { appPath };
}
