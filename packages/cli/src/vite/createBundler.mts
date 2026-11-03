// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { build } from "./build.mts";
import { hmr } from "./hmr.mts";
import { watch } from "./watch.mts";
import type { AdapterCore, Bundler, FrontendId } from "../adapterCore.mts";
import type { ViteIntegration } from "./integration.mts";

/**
 * The Vite adapter.
 * @param core - the CLI logic the adapter uses
 * @param frontend - the frontend to bundle: its package and its name there
 * @param integration - the frontend's Vite plugins and refused features
 */
export function createViteBundler(
  core: AdapterCore,
  frontend: FrontendId,
  integration: ViteIntegration,
): Bundler {
  return {
    watch: (options) => watch(core, frontend, integration, options),
    build: (options) => build(core, frontend, integration, options),
    hmr: (options) => hmr(core, frontend, integration, options),
  };
}
