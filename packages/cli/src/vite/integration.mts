// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { PluginOption } from "vite";

import { injectNames } from "./inject.mts";
import { requirePeers } from "./requirePeers.mts";
import { styleToElements } from "./style.mts";
import { rejectUnsupported } from "./unsupported.mts";
import type { AdapterCore, FrontendId } from "../adapterCore.mts";
import type { Inject } from "./inject.mts";
import type { UnsupportedFeature } from "./unsupported.mts";

/** What the Vite adapter needs of a frontend. */
export interface ViteIntegration {
  /** The package that lists the frontend's peers, and its key there. */
  frontend: FrontendId;
  /** The frontend's own plugins. Returns a fresh list on every call. */
  plugins(): PluginOption[];
  /** The module ids the build refuses. */
  unsupported: readonly UnsupportedFeature[];
  /** The free names every module binds to an import. */
  inject: Inject;
  /**
   * The module that exports `updateStyle(id, css)` and `removeStyle(id)`. A stylesheet module
   * calls them.
   */
  styleModule: string;
}

/**
 * The plugins that a frontend adds to a Vite config: its own, the ones that turn stylesheets into
 * calls of the style module, the peer check, the refusal of unsupported features and the import of
 * the injected names. Calls `integration.plugins()` once.
 * @param core - the CLI logic the adapter uses
 * @param integration - the frontend to bundle with its Vite plugins, style module and refused
 *   features
 * @param root - the root of the Vite project
 * @returns the plugins, in the order they run
 */
export function integrationPlugins(
  core: AdapterCore,
  integration: ViteIntegration,
  root: string,
): PluginOption[] {
  return [
    ...integration.plugins(),
    ...styleToElements(integration.styleModule, root),
    requirePeers(core, integration.frontend),
    rejectUnsupported(core, integration.unsupported),
    injectNames(integration.inject),
  ];
}
