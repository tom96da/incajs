// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import * as core from "../adapterCore.mts";
import { requirePeers } from "./requirePeers.mts";

type Context = { resolve: (...args: unknown[]) => Promise<unknown> };
type ResolveId = (
  this: Context,
  id: string,
  importer: string | undefined,
  options: object,
) => Promise<unknown>;

const PEERS = ["@vue/runtime-core", "@vue/runtime-dom"];
const resolveId = requirePeers(core).resolveId as unknown as ResolveId;

describe("requirePeers", () => {
  it("leaves another id to the resolver without resolving it", async () => {
    let asked = false;
    const context: Context = {
      resolve: async () => {
        asked = true;
        return null;
      },
    };

    const result = await resolveId.call(context, "left-pad", undefined, {});

    expect(result).toBeNull();
    expect(asked).toBe(false);
  });

  it.each(PEERS)("lets an installed %s through", async (peer) => {
    const context: Context = { resolve: async () => ({ id: `/node_modules/${peer}` }) };

    const result = await resolveId.call(context, peer, undefined, {});

    expect(result).toBeNull();
  });

  it.each(PEERS)("resolves %s again for the same importer, skipping itself", async (peer) => {
    let seen: unknown[] = [];
    const context: Context = {
      resolve: async (...args) => {
        seen = args;
        return { id: `/node_modules/${peer}` };
      },
    };

    await resolveId.call(context, peer, "/app/src/main.mts", { isEntry: false });

    expect(seen).toEqual([peer, "/app/src/main.mts", { isEntry: false, skipSelf: true }]);
  });

  it.each(PEERS)("throws the install command when %s does not resolve", async (peer) => {
    const context: Context = { resolve: async () => null };

    await expect(resolveId.call(context, peer, undefined, {})).rejects.toThrow(
      `ERR_INCA_PEER_NOT_FOUND: ${peer} is not installed. add it to the app: pnpm add ${peer}`,
    );
  });
});
