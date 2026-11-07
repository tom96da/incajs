// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { PluginOption } from "vite";

import { requirePeers } from "./requirePeers.mts";
import { styleToElements } from "./style.mts";
import { rejectUnsupported } from "./unsupported.mts";
import type { AdapterCore, FrontendId } from "../adapterCore.mts";
import type { UnsupportedFeature } from "./unsupported.mts";

/** What the Vite adapter needs of a frontend. */
export interface ViteIntegration {
  /** The frontend's own plugins. Returns a fresh list on every call. */
  plugins(): PluginOption[];
  /** The module ids the build refuses. */
  unsupported: readonly UnsupportedFeature[];
  /**
   * The module that exports `updateStyle(id, css)` and `removeStyle(id)`. A stylesheet module
   * calls them.
   */
  styleModule: string;
}

/**
 * The plugins that a frontend adds to a Vite config: its own, the ones that turn stylesheets into
 * calls of the style module, the peer check and the refusal of unsupported features. Calls
 * `integration.plugins()` once.
 * @param core - the CLI logic the adapter uses
 * @param frontend - the frontend to bundle
 * @param integration - the frontend's Vite plugins, style module and refused features
 * @param root - the root of the Vite project
 * @returns the plugins, in the order they run
 */
export function integrationPlugins(
  core: AdapterCore,
  frontend: FrontendId,
  integration: ViteIntegration,
  root: string,
): PluginOption[] {
  return [
    ...integration.plugins(),
    ...styleToElements(integration.styleModule, root),
    requirePeers(core, frontend),
    rejectUnsupported(core, integration.unsupported),
  ];
}
