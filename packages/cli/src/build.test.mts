// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { describe, expect, it } from "vitest";

import { build } from "./build.mts";
import type { Bundler, BuildOptions, BuildOutput, Watcher } from "./adapter/types.mts";

function makeFakeBundler(result: () => Promise<BuildOutput>): Bundler {
  return {
    watch: () => Promise.reject(new Error("not used by build()")),
    build: (): Promise<BuildOutput> => result(),
  };
}

describe("build", () => {
  it("builds through the bundler and returns what it wrote", async () => {
    let seen: BuildOptions | undefined;
    const bundler: Bundler = {
      watch: (): Promise<Watcher> => Promise.reject(new Error("not used by build()")),
      build: (options) => {
        seen = options;
        return Promise.resolve({
          outDir: options.outDir,
          entryFile: path.join(options.outDir, "bundle.js"),
          files: ["bundle.js"],
        });
      },
    };

    const output = await build({ cwd: "/app", entry: "unused", bundler });

    expect(seen).toEqual({ entry: "unused", outDir: "/app/dist" });
    expect(output.entryFile).toBe("/app/dist/bundle.js");
  });

  it("propagates a bundler build failure", async () => {
    const bundler = makeFakeBundler(() => Promise.reject(new Error("syntax error")));

    await expect(build({ cwd: "/app", entry: "unused", bundler })).rejects.toThrow("syntax error");
  });
});
