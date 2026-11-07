// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";
import type { Plugin } from "vite";

import { styleToElements } from "./style.mts";

const ROOT = "/app/src";
const SHEET = "/app/src/a.css";

type Transform = (code: string, id: string) => { code: string } | null;

/** The collect and the apply transform of a fresh plugin set. */
function steps() {
  const [collect, apply] = styleToElements("alpha-style", ROOT) as [Plugin, Plugin];
  return { collectOf: collect.transform as Transform, applyOf: apply.transform as Transform };
}

/** The apply transform after the collect step saw `id` with `css`. */
function applyAfterCollect(id: string, css = ".a { color: red }"): Transform {
  const { collectOf, applyOf } = steps();
  collectOf(css, id);
  return applyOf;
}

describe("inca:style-collect", () => {
  it("returns an empty text for a stylesheet id", () => {
    expect(steps().collectOf(".a {}", SHEET)).toEqual({ code: "", map: null });
  });

  it.each(["?inline", "?url", "?raw", "?direct", "?transform-only", "?worker", "?sharedworker"])(
    "leaves %s to Vite",
    (query) => {
      expect(steps().collectOf(".a {}", `${SHEET}${query}`)).toBeNull();
    },
  );

  it("leaves a module that is no stylesheet to Vite", () => {
    expect(steps().collectOf("export {}", "/app/src/a.ts")).toBeNull();
  });
});

describe("inca:style-apply", () => {
  it("appends the import and the call after the code of Vite", () => {
    const css = '.a::after { content: "</style>" }\n';
    const result = applyAfterCollect(SHEET, css)("export const x = 1;", SHEET);

    expect(result?.code).toBe(
      `export const x = 1;\nimport { updateStyle } from "alpha-style";\n` +
        `updateStyle("a.css", ${JSON.stringify(css)});\n`,
    );
  });

  it("keys a sheet by its path below the root with its query", () => {
    const id = `${ROOT}/b/App.vue?vue&type=style&index=0&scoped=x&lang.css`;
    const result = applyAfterCollect(id)("", id);

    expect(result?.code).toContain(
      'updateStyle("b/App.vue?vue&type=style&index=0&scoped=x&lang.css"',
    );
  });

  it("keeps the path of a sheet outside the root", () => {
    const id = "/lib/c.css";

    expect(applyAfterCollect(id)("", id)?.code).toContain('updateStyle("/lib/c.css"');
  });

  it("keeps a file named like a parent path inside the root", () => {
    const id = "/app/src/..x.css";

    expect(applyAfterCollect(id)("", id)?.code).toContain('updateStyle("..x.css"');
  });

  it("leaves an id the collect step did not see", () => {
    expect(steps().applyOf("", SHEET)).toBeNull();
  });
});
