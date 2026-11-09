// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { transformWithOxc } from "vite";
import type { Plugin } from "vite";

/** The free names the oxc transform binds to an import, as it declares them. */
export type Inject = NonNullable<NonNullable<Parameters<typeof transformWithOxc>[2]>["inject"]>;

const EXTENSIONS = new Set(["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx", "vue"]);

/**
 * Adds an import for each free name of `inject` that a module uses. Runs in the build and in the
 * development server, after the Vue and TypeScript transforms.
 *
 * @param inject - the names to bind and the modules they come from
 */
export function injectNames(inject: Inject): Plugin {
  return {
    name: "inca:inject",
    enforce: "post",
    async transform(code, id) {
      const extension = id.split("?")[0]!.split(".").pop()!;
      if (!EXTENSIONS.has(extension)) return null;
      if (!Object.keys(inject).some((name) => code.includes(name))) return null;
      const result = await transformWithOxc(code, id, { lang: "js", inject });
      return { code: result.code, map: result.map };
    },
  };
}
