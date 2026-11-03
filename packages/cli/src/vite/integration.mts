// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { PluginOption } from "vite";

import { requirePeers } from "./requirePeers.mts";
import { rejectUnsupported } from "./unsupported.mts";
import type { AdapterCore, FrontendId } from "../adapterCore.mts";
import type { UnsupportedFeature } from "./unsupported.mts";

/** What the Vite adapter needs of a frontend. */
export interface ViteIntegration {
  /** The frontend's own plugins. Returns a fresh list on every call. */
  plugins(): PluginOption[];
  /** The module ids the build refuses, checked before the built-in ones. */
  unsupported: readonly UnsupportedFeature[];
}

/**
 * The plugins that a frontend adds to a Vite config: its own, the peer check and the refusal of
 * unsupported features. Calls `integration.plugins()` once.
 * @param core - the CLI logic the adapter uses
 * @param frontend - the frontend to bundle
 * @param integration - the frontend's Vite plugins and refused features
 * @returns the plugins, in the order they run
 */
export function integrationPlugins(
  core: AdapterCore,
  frontend: FrontendId,
  integration: ViteIntegration,
): PluginOption[] {
  return [
    ...integration.plugins(),
    requirePeers(core, frontend),
    rejectUnsupported(core, integration.unsupported),
  ];
}
