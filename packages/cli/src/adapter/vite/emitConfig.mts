// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Plugin } from "vite";

/** The file a build writes its app's config to, beside the entry. */
export const CONFIG_FILE_NAME = "inca.json";

/**
 * Serializes `config` the way `inca.json` is written: `undefined` for
 * anything that shouldn't produce a file — `undefined` itself, or an
 * object with no keys.
 */
export function serializeConfig(config: unknown): string | undefined {
  const serialized = JSON.stringify(config, undefined, 2);
  if (serialized === undefined || serialized === "{}") return undefined;
  return `${serialized}\n`;
}

/**
 * Writes `config` to `inca.json` beside the entry, for `inca-host` to read
 * when it starts the app.
 *
 * @param config - any JSON-serializable value; no file is written for
 * `undefined` or an object with no keys
 */
export function emitConfig(config: unknown): Plugin {
  const content = serializeConfig(config);

  return {
    name: "inca:emit-config",
    generateBundle() {
      if (!content) return;
      this.emitFile({ type: "asset", fileName: CONFIG_FILE_NAME, source: content });
    },
  };
}
