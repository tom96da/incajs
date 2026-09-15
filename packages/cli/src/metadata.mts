// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";

/** An app's own `package.json`, the parts `gpjsui package` cares about. */
interface AppPackageJson {
  name?: string;
  version?: string;
  description?: string;
  gpjsui?: {
    productName?: string;
    identifier?: string;
    icon?: string;
  };
}

/** An app's identity and version, for `gpjsui package` to stamp onto the application it emits. */
export interface AppMetadata {
  /** Shown to the user — the app's display name. */
  productName: string;
  /** A reverse-DNS-style unique id — e.g. macOS's `CFBundleIdentifier`. */
  identifier: string;
  /** True when `identifier` was derived rather than set by the app. */
  identifierIsDefault: boolean;
  /** The packaged app's version. */
  version: string;
  /** Absolute path to an icon file, if the app declared one. */
  icon?: string;
}

/** Strips a package name's scope, e.g. `incajs/vue` → `vue`. */
function unscopedName(name: string): string {
  const slash = name.indexOf("/");
  return name.startsWith("@") && slash !== -1 ? name.slice(slash + 1) : name;
}

/**
 * Turns a display name into a filesystem/URL-safe slug — lowercased, with
 * anything outside `[a-z0-9]` collapsed into a single `-`. Used for the
 * default `identifier` and for the Linux packaged-app directory name.
 */
export function slugify(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

/**
 * Reads an app's package metadata for `gpjsui package`: `productName`,
 * `identifier`, `version`, and an optional `icon`. Everything is derived
 * from the app's own `package.json` — its `name`/`version` fields, and an
 * optional `"gpjsui"` key overriding any of them:
 * `{ "gpjsui": { "productName", "identifier", "icon" } }`.
 *
 * @throws if `package.json` is missing, has neither a `name` nor a
 * `"gpjsui".productName`, or `"gpjsui".icon` doesn't resolve to a real file.
 */
export async function readAppMetadata(cwd: string): Promise<AppMetadata> {
  const pkgPath = path.join(cwd, "package.json");
  let raw: string;
  try {
    raw = await readFile(pkgPath, "utf8");
  } catch {
    throw new Error(`no package.json found at ${pkgPath}`);
  }

  const pkg = JSON.parse(raw) as AppPackageJson;
  const config = pkg.gpjsui ?? {};

  const productName = config.productName ?? (pkg.name ? unscopedName(pkg.name) : undefined);
  if (!productName) {
    throw new Error(`${pkgPath} needs a "name", or "gpjsui": { "productName" }`);
  }

  const version = pkg.version ?? "0.0.0";

  const identifier = config.identifier ?? `org.gpjsui.${slugify(productName)}`;

  let icon: string | undefined;
  if (config.icon) {
    icon = path.resolve(cwd, config.icon);
    if (!existsSync(icon)) {
      throw new Error(`"gpjsui.icon" points to ${icon}, which doesn't exist`);
    }
  }

  return {
    productName,
    identifier,
    identifierIsDefault: config.identifier === undefined,
    version,
    icon,
  };
}
