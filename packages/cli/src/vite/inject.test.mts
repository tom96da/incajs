// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { injectNames } from "./inject.mts";

const plugin = injectNames({ document: ["alpha-pkg", "document"] });
type Transform = (code: string, id: string) => Promise<{ code: string } | null>;
const transform = plugin.transform as Transform;
type Config = () => { resolve: { dedupe: string[] } };

describe("injectNames", () => {
  it("adds the import to a module that names the injected name", async () => {
    const result = await transform("export const a = document.title;", "/app/a.js");

    expect(result!.code).toContain('import { document } from "alpha-pkg"');
  });

  it("transforms a module id that has a query", async () => {
    const result = await transform("export const a = document.title;", "/app/a.js?v=1");

    expect(result!.code).toContain('import { document } from "alpha-pkg"');
  });

  it("passes a module of another extension through", async () => {
    expect(await transform("a { color: red }", "/app/a.css")).toBeNull();
  });

  it("passes a module that names none of the injected names through", async () => {
    expect(await transform("export const a = 1;", "/app/a.js")).toBeNull();
  });

  it.each(["mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx", "vue"])(
    "transforms a .%s module",
    async (extension) => {
      const result = await transform("export const a = document.title;", `/app/a.${extension}`);

      expect(result!.code).toContain('import { document } from "alpha-pkg"');
    },
  );

  it("resolves the injected modules to one copy", () => {
    expect((plugin.config as Config)().resolve.dedupe).toEqual(["alpha-pkg"]);
  });

  it("runs after the Vue and TypeScript transforms", () => {
    expect(plugin.enforce).toBe("post");
  });

  it("runs in the build and in the development server", () => {
    expect(plugin.apply).toBeUndefined();
  });
});
