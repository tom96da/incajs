// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Plugin } from "vite";

/** The file a build writes its app's config to, beside the entry. */
export const CONFIG_FILE_NAME = "inca.json";

/**
 * Writes `config` to `inca.json` beside the entry, for the host to read
 * when it starts the app.
 *
 * @param config - any JSON-serializable value; no file is written for
 * `undefined`
 */
export function emitConfig(config: unknown): Plugin {
  const serialized = JSON.stringify(config, undefined, 2);

  return {
    name: "inca:emit-config",
    generateBundle() {
      if (serialized === undefined) return;
      this.emitFile({ type: "asset", fileName: CONFIG_FILE_NAME, source: `${serialized}\n` });
    },
  };
}
