// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { IncaConfig } from "./types.mts";

/**
 * The `IncaConfig` fields whose default is a fixed value rather than one
 * derived from `package.json` or the filesystem — importable alongside
 * `defineConfig` for an app that wants to inherit or reference them.
 *
 * @example
 * ```ts
 * import { defaultConfig, defineConfig } from "@incajs/cli/config";
 *
 * export default defineConfig({
 *   ...defaultConfig,
 *   outDir: "build",
 * });
 * ```
 */
export const defaultConfig: Required<Pick<IncaConfig, "outDir" | "version">> = {
  outDir: "dist",
  version: "0.0.0",
};

/**
 * Strips an npm scope off a package name.
 *
 * @param name - a `package.json` `"name"`, e.g. `@incajs/cli` or `incajs`
 * @returns the name with its scope removed, e.g. `cli`; an unscoped name
 * passes through unchanged
 */
export function unscopedName(name: string): string {
  const slash = name.indexOf("/");
  return name.startsWith("@") && slash !== -1 ? name.slice(slash + 1) : name;
}

/**
 * Turns a display name into a filesystem/URL-safe slug.
 *
 * @param name - e.g. a `productName`
 * @returns `name` lowercased, with runs of non-`[a-z0-9]` collapsed to a
 * single `-` and trimmed from both ends
 */
export function slugify(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

/**
 * @param pkgName - `package.json`'s own `"name"`, if it has one
 * @returns the `productName` a plain `package.json` implies — its name with
 * any scope stripped — or `undefined` if it has none
 */
export function defaultProductName(pkgName: string | undefined): string | undefined {
  return pkgName ? unscopedName(pkgName) : undefined;
}

/**
 * @param productName - the app's resolved display name
 * @returns a generated `identifier`, e.g. `org.inca.click-counter`
 */
export function defaultIdentifier(productName: string): string {
  return `org.inca.${slugify(productName)}`;
}
