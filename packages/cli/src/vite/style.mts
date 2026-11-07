// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import dedent from "ts-dedent";
import { isCSSRequest } from "vite";
import type { Plugin } from "vite";

/** Imports that Vite turns into something other than a stylesheet of the app. */
const OTHER_IMPORT_RE =
  /[?&](?:inline|url|raw|direct|transform-only|worker|sharedworker)(?:[&=]|$)/;

/**
 * The key of a stylesheet: its path below `root` with its query, `/`-separated. A path outside
 * `root` stays as it is.
 */
function keyOf(id: string, root: string): string {
  const [file = id, ...query] = id.split("?");
  const relative = path.relative(root, file);
  const inside = !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
  const key = (inside ? relative : file).split(path.sep).join("/");
  return query.length > 0 ? `${key}?${query.join("?")}` : key;
}

/**
 * The plugins that turn the CSS of a build into calls of `updateStyle`. Vite emits no
 * stylesheet file for them.
 * @param styleModule - the module that exports `updateStyle`
 * @param root - the root of the Vite project, the base of the keys
 * @returns the plugins, in the order they run
 */
export function styleToElements(styleModule: string, root: string): Plugin[] {
  const sheets = new Map<string, string>();
  return [
    {
      name: "inca:style-collect",
      apply: "build",
      transform(css, id) {
        if (!isCSSRequest(id) || OTHER_IMPORT_RE.test(id)) return null;
        sheets.set(id, css);
        return { code: "", map: null };
      },
    },
    {
      name: "inca:style-apply",
      apply: "build",
      enforce: "post",
      transform(code, id) {
        const css = sheets.get(id);
        if (css === undefined) return null;
        return {
          code:
            dedent`
            ${code}
            import { updateStyle } from ${JSON.stringify(styleModule)};
            updateStyle(${JSON.stringify(keyOf(id, root))}, ${JSON.stringify(css)});
          ` + "\n",
          map: null,
          moduleSideEffects: "no-treeshake",
        };
      },
    },
  ];
}
