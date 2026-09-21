// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * An app's own `inca.config.ts` (or `.js`/`.json`), loaded by `@incajs/cli`.
 * `package.json`'s `"inca"` key still works as a fallback, but is
 * deprecated — this file wins where both are present.
 */
export interface IncaConfig {
  /** Path to an icon file, resolved relative to this config file's own directory. */
  icon?: string;
  /**
   * A reverse-DNS-style unique id — e.g. macOS's `CFBundleIdentifier`.
   * @default a generated `org.inca.<slug>`
   */
  identifier?: string;
  /**
   * The app's display name.
   * @default package.json's "name", with any npm scope stripped
   */
  productName?: string;
  /**
   * The packaged app's version.
   * @default package.json's own "version"
   */
  version?: string;
  /**
   * The app's entry point.
   * @default resolved automatically — a committed `src/main.mts`, or
   * `src/App.vue` wrapped in a synthesized one
   */
  entry?: string;
  /**
   * Where a build's output is written.
   * @default "dist"
   */
  outDir?: string;
  /** The window the app opens in. */
  window?: WindowConfig;
}

/** What an app declares about its window — see {@link IncaConfig.window}. */
export interface WindowConfig {
  /**
   * Initial width, in pixels. Falls back to the width the app's root
   * element declares, then to this default.
   * @default 800
   */
  width?: number;
  /**
   * Initial height, in pixels. Falls back to the height the app's root
   * element declares, then to this default.
   * @default 600
   */
  height?: number;
  /**
   * The window's title.
   * @default the app's productName
   */
  title?: string;
}
