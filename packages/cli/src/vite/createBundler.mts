// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { build } from "./build.mts";
import { hmr } from "./hmr.mts";
import { watch } from "./watch.mts";
import type { AdapterCore, Bundler } from "../adapterCore.mts";
import type { ViteIntegration } from "./integration.mts";

/**
 * The Vite adapter.
 * @param core - the CLI logic the adapter uses
 * @param integration - the frontend to bundle with its Vite plugins, style module and refused
 *   features
 */
export function createViteBundler(core: AdapterCore, integration: ViteIntegration): Bundler {
  return {
    watch: (options) => watch(core, integration, options),
    build: (options) => build(core, integration, options),
    hmr: (options) => hmr(core, integration, options),
  };
}
