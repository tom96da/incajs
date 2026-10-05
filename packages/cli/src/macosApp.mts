// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { chmod, cp, link, mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { encodePlist } from "./plist.mts";
import type { PlistValue } from "./plist.mts";

/** The app metadata a `.app` bundle's `Info.plist` carries. */
export interface MacosAppMetadata {
  /** The name macOS shows in the Dock, the menu bar and the Finder. */
  productName: string;
  /** A reverse-DNS-style unique id, written as `CFBundleIdentifier`. */
  identifier: string;
  /** The app's version, written as both `CFBundleVersion` and `CFBundleShortVersionString`. */
  version: string;
  /** Absolute path to an icon file, copied into `Contents/Resources`. */
  icon?: string;
}

/** Options for {@link writeMacosApp}. */
export interface WriteMacosAppOptions {
  /** Where the `.app` directory is written. An existing one there is replaced. */
  appPath: string;
  /** What the bundle's `Info.plist` describes. */
  metadata: MacosAppMetadata;
  /** The `inca-host` binary the bundle's executable is made from. */
  hostBin: string;
  /**
   * Hard-links the executable to `hostBin` rather than copying it, and
   * copies when the two paths are on different filesystems. Set it where
   * the bundle is rebuilt often and holds no files of its own.
   */
  link?: boolean;
}

/** The paths {@link writeMacosApp} created. */
export interface MacosApp {
  /** The `.app` directory itself. */
  appPath: string;
  /** The bundle's executable, at `Contents/MacOS/<productName>`. */
  executablePath: string;
  /** `Contents/Resources`, where an app's own files belong. */
  resourcesDir: string;
}

/**
 * Writes a macOS `.app` bundle around the `inca-host` binary:
 * `Contents/MacOS` holding the executable, `Contents/Resources` left empty
 * apart from the icon, and `Info.plist`/`PkgInfo` beside them. macOS reads
 * the app's name, icon and identifier from the bundle, so an executable
 * launched from one carries them wherever the platform shows an app.
 *
 * @param options - where to write, what to describe, and which binary to use
 * @returns the bundle's own path, its executable, and its resources directory
 * @throws if any part of the bundle can't be written
 */
export async function writeMacosApp(options: WriteMacosAppOptions): Promise<MacosApp> {
  const { appPath, metadata, hostBin } = options;
  await rm(appPath, { recursive: true, force: true });

  const contentsDir = path.join(appPath, "Contents");
  const macosDir = path.join(contentsDir, "MacOS");
  const resourcesDir = path.join(contentsDir, "Resources");
  await mkdir(macosDir, { recursive: true });
  await mkdir(resourcesDir, { recursive: true });

  const executablePath = path.join(macosDir, metadata.productName);
  if (options.link) {
    await link(hostBin, executablePath).catch(() => cp(hostBin, executablePath));
  } else {
    await cp(hostBin, executablePath);
  }
  await chmod(executablePath, 0o755);

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

  return { appPath, executablePath, resourcesDir };
}
