// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { build } from "./build.mts";
import { hmr } from "./hmr.mts";
import { watch } from "./watch.mts";
import type { AdapterCore, Bundler } from "../adapterCore.mts";
import type { FrontendOptions, FrontendId } from "./config.mts";

/**
 * The Vite adapter.
 * @param core - the CLI logic the adapter uses
 * @param frontend - the frontend to bundle: its package and its name there
 * @param frontendOptions - the frontend's own settings
 */
export function createViteBundler(
  core: AdapterCore,
  frontend: FrontendId,
  frontendOptions: FrontendOptions,
): Bundler {
  return {
    watch: (options) => watch(core, frontend, frontendOptions, options),
    build: (options) => build(core, frontend, frontendOptions, options),
    hmr: (options) => hmr(core, frontend, frontendOptions, options),
  };
}
