// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

/**
 * Records the app's root in a runtime config.
 *
 * @param config - the runtime config; a value that is not an object counts as an empty one
 * @param appRoot - the app's root directory
 * @param configDir - the directory the config is written to, as `inca.json`
 * @returns a copy of `config` whose `root` is `appRoot` relative to `configDir`, `/`-separated
 */
export function withAppRoot(
  config: unknown,
  appRoot: string,
  configDir: string,
): Record<string, unknown> {
  const members = typeof config === "object" && config !== null ? config : {};
  const root = path.relative(configDir, appRoot).split(path.sep).join("/");
  return { ...members, root: root === "" ? "." : root };
}
