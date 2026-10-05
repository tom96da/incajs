// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { build } from "./build.mts";
import { hmr } from "./hmr.mts";
import { watch } from "./watch.mts";
import type { AdapterCore } from "../../adapterCore.mts";
import type { Bundler } from "../types.mts";

/**
 * The Vite adapter.
 * @param core - the CLI logic the adapter uses
 * @param runtimeModuleName - the module compiled `.vue` templates import their helpers from
 */
export function createViteBundler(core: AdapterCore, runtimeModuleName: string): Bundler {
  return {
    watch: (options) => watch(core, runtimeModuleName, options),
    build: (options) => build(core, runtimeModuleName, options),
    hmr: (options) => hmr(core, runtimeModuleName, options),
  };
}
