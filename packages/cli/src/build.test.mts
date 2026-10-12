// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { describe, expect, it, vi } from "vitest";

import { build } from "./build.mts";
import type { Bundler, BuildOptions, BuildOutput } from "./adapter/types.mts";

type RecordingBundler = Bundler & { seen: BuildOptions | undefined };

function makeFakeBundler(
  result: (options: BuildOptions) => Promise<BuildOutput>,
): RecordingBundler {
  const bundler: RecordingBundler = {
    seen: undefined,
    watch: () => Promise.reject(new Error("not used by build()")),
    build: (options) => {
      bundler.seen = options;
      return result(options);
    },
  };
  return bundler;
}

const writeNothing = (options: BuildOptions): Promise<BuildOutput> =>
  Promise.resolve({ outDir: options.outDir, entryFile: "bundle.js", files: [] });

describe("build", () => {
  it("builds through the bundler and returns what it wrote", async () => {
    const bundler = makeFakeBundler((options) =>
      Promise.resolve({
        outDir: options.outDir,
        entryFile: path.join(options.outDir, "bundle.js"),
        files: ["bundle.js"],
      }),
    );

    const output = await build({ cwd: "/app", entry: "unused", bundler });

    expect(bundler.seen).toEqual({ entry: "unused", outDir: "/app/dist", sourcemap: false });
    expect(output.entryFile).toBe("/app/dist/bundle.js");
  });

  it("refuses a supplied config whose outDir holds the app, without building", async () => {
    const buildSpy = vi.fn<Bundler["build"]>();
    const bundler: Bundler = { watch: () => Promise.reject(new Error("unused")), build: buildSpy };

    await expect(
      build({
        cwd: "/app",
        entry: "unused",
        bundler,
        config: { outDir: "/app", entry: undefined, sourcemap: false },
      }),
    ).rejects.toThrow(expect.objectContaining({ code: "ERR_INCA_OUT_DIR_INVALID" }));
    expect(buildSpy).not.toHaveBeenCalled();
  });

  it("refuses a relative outDir that holds the app when cwd is not the process cwd", async () => {
    const buildSpy = vi.fn<Bundler["build"]>();
    const bundler: Bundler = { watch: () => Promise.reject(new Error("unused")), build: buildSpy };

    await expect(
      build({
        cwd: "/app",
        entry: "/app/src/main.mts",
        bundler,
        config: { outDir: ".", entry: undefined, sourcemap: false },
      }),
    ).rejects.toThrow(expect.objectContaining({ code: "ERR_INCA_OUT_DIR_INVALID" }));
    expect(buildSpy).not.toHaveBeenCalled();
  });

  it("passes sourcemap: true when the option overrides a config that leaves it off", async () => {
    const bundler = makeFakeBundler(writeNothing);

    await build({
      cwd: "/app",
      entry: "unused",
      bundler,
      sourcemap: true,
      config: { outDir: "/app/dist", entry: undefined, sourcemap: false },
    });

    expect(bundler.seen?.sourcemap).toBe(true);
  });

  it("lets an explicit sourcemap option turn off a config that turns it on", async () => {
    const bundler = makeFakeBundler(writeNothing);

    await build({
      cwd: "/app",
      entry: "unused",
      bundler,
      sourcemap: false,
      config: { outDir: "/app/dist", entry: undefined, sourcemap: true },
    });

    expect(bundler.seen?.sourcemap).toBe(false);
  });

  it("uses the config's sourcemap when no option is given", async () => {
    const bundler = makeFakeBundler(writeNothing);

    await build({
      cwd: "/app",
      entry: "unused",
      bundler,
      config: { outDir: "/app/dist", entry: undefined, sourcemap: true },
    });

    expect(bundler.seen?.sourcemap).toBe(true);
  });

  it("propagates a bundler build failure", async () => {
    const bundler = makeFakeBundler(() => Promise.reject(new Error("syntax error")));

    await expect(build({ cwd: "/app", entry: "unused", bundler })).rejects.toThrow("syntax error");
  });
});
