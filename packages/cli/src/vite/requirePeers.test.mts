// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import * as core from "../adapterCore.mts";
import { requirePeers } from "./requirePeers.mts";

type Context = { resolve: (...args: unknown[]) => Promise<unknown> };
type ResolveId = (
  this: Context,
  id: string,
  importer: string | undefined,
  options: object,
) => Promise<unknown>;

const PKG = "alpha-pkg";
const FRONTEND = [PKG, "alpha"] as const;
const PEERS = ["@alpha/core", "@alpha/dom"];
/** A package.json whose `incajs.frontends` lists `peers` for each frontend. */
const listing = (frontends: Record<string, string[]>): object => ({
  incajs: {
    frontends: Object.fromEntries(Object.entries(frontends).map(([k, peers]) => [k, { peers }])),
  },
});
const marked = listing({ alpha: PEERS });
let dir: string | undefined;

afterEach(async () => {
  if (dir) await rm(dir, { recursive: true, force: true });
  dir = undefined;
});

/** A plugin whose Vite root holds a fake `alpha-pkg` with the given contents. */
async function pluginFor(contents?: object): Promise<ResolveId> {
  dir = await mkdtemp(path.join(tmpdir(), "inca-peers-"));
  if (contents) {
    const packageDir = path.join(dir, "node_modules", PKG);
    await mkdir(packageDir, { recursive: true });
    await writeFile(
      path.join(packageDir, "package.json"),
      JSON.stringify({ name: PKG, ...contents }),
    );
  }
  const plugin = requirePeers(core, FRONTEND);
  (plugin.configResolved as (config: { root: string }) => void)({ root: dir });
  return plugin.resolveId as unknown as ResolveId;
}

const unresolved: Context = { resolve: async () => null };

describe("requirePeers", () => {
  it("leaves another id to the resolver without resolving it", async () => {
    const resolveId = await pluginFor(marked);
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
    const resolveId = await pluginFor(marked);
    const context: Context = { resolve: async () => ({ id: `/node_modules/${peer}` }) };

    const result = await resolveId.call(context, peer, undefined, {});

    expect(result).toBeNull();
  });

  it.each(PEERS)("resolves %s again for the same importer, skipping itself", async (peer) => {
    const resolveId = await pluginFor(marked);
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
    const resolveId = await pluginFor(marked);

    await expect(resolveId.call(unresolved, peer, undefined, {})).rejects.toThrow(
      `ERR_INCA_PEER_NOT_FOUND: ${peer} can't be resolved. add it to the app's dependencies`,
    );
  });

  it("throws the install command when Vite stands in for an optional peer", async () => {
    const resolveId = await pluginFor(marked);
    const context: Context = {
      resolve: async () => ({ id: `__vite-optional-peer-dep:${PEERS[0]}:${PKG}` }),
    };

    await expect(resolveId.call(context, PEERS[0]!, undefined, {})).rejects.toThrow(
      `ERR_INCA_PEER_NOT_FOUND: ${PEERS[0]} can't be resolved.`,
    );
  });

  it("doesn't check a peer no frontend lists", async () => {
    const resolveId = await pluginFor(marked);

    await expect(resolveId.call(unresolved, "left", undefined, {})).resolves.toBeNull();
  });

  it("doesn't check a peer another frontend lists", async () => {
    const resolveId = await pluginFor(listing({ beta: ["@beta/core"] }));

    await expect(resolveId.call(unresolved, "@beta/core", undefined, {})).resolves.toBeNull();
  });

  it("throws the install command when the frontend's package is missing", async () => {
    const resolveId = await pluginFor();

    await expect(resolveId.call(unresolved, PEERS[0]!, undefined, {})).rejects.toMatchObject({
      code: "ERR_INCA_PEER_NOT_FOUND",
      message: `ERR_INCA_PEER_NOT_FOUND: ${PKG} can't be resolved. add it to the app's dependencies`,
    });
  });

  it("fails when the package.json of the frontend's package can't be read", async () => {
    const resolveId = await pluginFor(marked);
    const packageJSONPath = path.join(dir!, "node_modules", PKG, "package.json");
    await rm(packageJSONPath);
    await mkdir(packageJSONPath);

    await expect(resolveId.call(unresolved, PEERS[0]!, undefined, {})).rejects.toMatchObject({
      code: "EISDIR",
    });
  });

  it("reads the package.json of the named package only", async () => {
    const resolveId = await pluginFor(marked);
    const other = requirePeers(core, ["beta-pkg", FRONTEND[1]]);
    (other.configResolved as (config: { root: string }) => void)({ root: dir! });

    await expect(
      (other.resolveId as unknown as ResolveId).call(unresolved, PEERS[0]!, undefined, {}),
    ).rejects.toThrow("beta-pkg can't be resolved");
    await expect(resolveId.call(unresolved, PEERS[0]!, undefined, {})).rejects.toThrow(
      "ERR_INCA_PEER_NOT_FOUND",
    );
  });

  it("checks nothing when package.json has no incajs key", async () => {
    const resolveId = await pluginFor();
    const packageDir = path.join(dir!, "node_modules", PKG);
    await mkdir(packageDir, { recursive: true });
    await writeFile(path.join(packageDir, "package.json"), "{}");

    await expect(resolveId.call(unresolved, PEERS[0]!, undefined, {})).resolves.toBeNull();
  });

  it("checks nothing for a frontend the package doesn't list", async () => {
    const resolveId = await pluginFor(listing({ beta: PEERS }));

    await expect(resolveId.call(unresolved, PEERS[0]!, undefined, {})).resolves.toBeNull();
  });

  it("checks nothing when the frontend lists no peers", async () => {
    const resolveId = await pluginFor({ incajs: { frontends: { alpha: {} } } });

    await expect(resolveId.call(unresolved, PEERS[0]!, undefined, {})).resolves.toBeNull();
  });

  it("checks nothing when the package lists no frontends", async () => {
    const resolveId = await pluginFor({ incajs: {} });

    await expect(resolveId.call(unresolved, PEERS[0]!, undefined, {})).resolves.toBeNull();
  });

  it("reads package.json again after a failed read", async () => {
    const resolveId = await pluginFor();
    await expect(resolveId.call(unresolved, PEERS[0]!, undefined, {})).rejects.toThrow(
      `${PKG} can't be resolved`,
    );
    const packageDir = path.join(dir!, "node_modules", PKG);
    await mkdir(packageDir, { recursive: true });
    await writeFile(
      path.join(packageDir, "package.json"),
      JSON.stringify({ incajs: { frontends: { [FRONTEND[1]]: { peers: PEERS } } } }),
    );

    await expect(resolveId.call(unresolved, PEERS[0]!, undefined, {})).rejects.toThrow(
      `${PEERS[0]} can't be resolved`,
    );
  });

  it("reads package.json once for several ids", async () => {
    const resolveId = await pluginFor(marked);
    const context: Context = { resolve: async () => ({ id: "x" }) };
    await resolveId.call(context, PEERS[0]!, undefined, {});
    await rm(path.join(dir!, "node_modules"), { recursive: true });

    await expect(resolveId.call(unresolved, PEERS[1]!, undefined, {})).rejects.toThrow(
      "ERR_INCA_PEER_NOT_FOUND",
    );
  });
});
