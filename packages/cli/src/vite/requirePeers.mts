// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

import type { Plugin } from "vite";

import type { AdapterCore } from "../adapterCore.mts";
import type { FrontendId } from "./config.mts";

/** Vite stands in for an optional peer it can't find with an id of its own. */
const OPTIONAL_PEER_PREFIX = "__vite-optional-peer-dep:";

/** The error for the package `id` that can't be resolved. */
function unresolvable(core: AdapterCore, id: string): Error {
  // The code is repeated in the message: the bundler re-wraps the
  // error and keeps only its text.
  const code = "ERR_INCA_PEER_NOT_FOUND";
  return new core.IncaError(
    code,
    `${code}: ${id} can't be resolved. add it to the app's dependencies`,
  );
}

/**
 * The text of the `package.json` of `pkg`, found in the `node_modules` directories that
 * Node searches from `root`. A package's `exports` doesn't matter here. Without `pkg` there
 * is nothing to read.
 */
async function readPackageJSON(root: string, pkg: string): Promise<string | undefined> {
  const searched = createRequire(pathToFileURL(`${root}/`)).resolve.paths(pkg) ?? [];
  for (const dir of searched) {
    try {
      return await readFile(path.join(dir, pkg, "package.json"), "utf8");
    } catch (error) {
      const { code } = error as NodeJS.ErrnoException;
      if (code !== "ENOENT" && code !== "ENOTDIR") throw error;
    }
  }
  return undefined;
}

/** The peers that the package `pkg` lists for `frontend`. Fails when `pkg` can't be resolved. */
async function markedPeers(
  core: AdapterCore,
  root: string,
  [pkg, frontend]: FrontendId,
): Promise<string[]> {
  const text = await readPackageJSON(root, pkg);
  if (text === undefined) throw unresolvable(core, pkg);
  const packageJSON = JSON.parse(text) as {
    incajs?: { frontends?: Record<string, { peers?: string[] }> };
  };
  return packageJSON.incajs?.frontends?.[frontend]?.peers ?? [];
}

/**
 * Fails the build with the install command when the app can't resolve a peer that the
 * frontend's package lists.
 */
export function requirePeers(core: AdapterCore, frontend: FrontendId): Plugin {
  let root = process.cwd();
  let peers: Promise<string[]> | undefined;
  return {
    name: "inca:require-peers",
    enforce: "pre",
    configResolved(config) {
      root = config.root;
    },
    async resolveId(id, importer, options) {
      peers ??= markedPeers(core, root, frontend);
      if (!(await peers).includes(id)) return null;
      const resolved = await this.resolve(id, importer, { ...options, skipSelf: true });
      if (resolved && !resolved.id.startsWith(OPTIONAL_PEER_PREFIX)) return null;
      throw unresolvable(core, id);
    },
  };
}
