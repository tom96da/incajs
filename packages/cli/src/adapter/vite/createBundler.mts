// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { build } from "./build.mts";
import { hmr } from "./hmr.mts";
import { watch } from "./watch.mts";
import type { AdapterCore } from "../../adapterCore.mts";
import type { Bundler } from "../types.mts";

/** The Vite adapter. */
export function createViteBundler(core: AdapterCore): Bundler {
  return {
    watch: (options) => watch(core, options),
    build: (options) => build(core, options),
    hmr: (options) => hmr(core, options),
  };
}
