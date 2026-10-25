// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Plugin } from "vite";

import type { AdapterCore } from "../adapterCore.mts";

const CODE = "ERR_INCA_PEER_NOT_FOUND";
const PEERS = ["@vue/runtime-core", "@vue/runtime-dom"];

/**
 * Fails the build with the install command when the app can't resolve
 * an optional Vue peer of `incajs`, `@vue/runtime-core` or `@vue/runtime-dom`.
 */
export function requirePeers(core: AdapterCore): Plugin {
  return {
    name: "inca:require-peers",
    enforce: "pre",
    async resolveId(id, importer, options) {
      if (!PEERS.includes(id)) return null;
      if (await this.resolve(id, importer, { ...options, skipSelf: true })) return null;
      // The code is repeated in the message: the bundler re-wraps the
      // error and keeps only its text.
      throw new core.IncaError(
        CODE,
        `${CODE}: ${id} is not installed. add it to the app: pnpm add ${id}`,
      );
    },
  };
}
