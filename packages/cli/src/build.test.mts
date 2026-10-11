// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { describe, expect, it, vi } from "vitest";

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

    expect(seen).toEqual({ entry: "unused", outDir: "/app/dist", sourcemap: false });
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

  it("writes bundle.js.map when the sourcemap option overrides a config that leaves it off", async () => {
    const scratch = await mkdtemp(path.join(tmpdir(), "inca-build-map-"));
    try {
      const app = path.join(scratch, "app");
      await mkdir(path.join(app, "src"), { recursive: true });
      const entry = path.join(app, "src", "main.ts");
      await writeFile(entry, 'console.log("hello");\n');
      const outDir = path.join(scratch, "dist");

      const output = await build({
        cwd: app,
        entry,
        sourcemap: true,
        config: { outDir, entry: undefined, sourcemap: false },
      });

      expect(output.files).toContain("bundle.js.map");
      await expect(stat(path.join(outDir, "bundle.js.map"))).resolves.toBeDefined();
      const js = await readFile(path.join(outDir, "bundle.js"), "utf8");
      expect(js.trimEnd().endsWith("//# sourceMappingURL=bundle.js.map")).toBe(true);
    } finally {
      await rm(scratch, { recursive: true, force: true });
    }
  });

  it("lets an explicit sourcemap option turn off a config that turns it on", async () => {
    let seen: BuildOptions | undefined;
    const bundler: Bundler = {
      watch: (): Promise<Watcher> => Promise.reject(new Error("not used by build()")),
      build: (options) => {
        seen = options;
        return Promise.resolve({ outDir: options.outDir, entryFile: "bundle.js", files: [] });
      },
    };

    await build({
      cwd: "/app",
      entry: "unused",
      bundler,
      sourcemap: false,
      config: { outDir: "/app/dist", entry: undefined, sourcemap: true },
    });

    expect(seen?.sourcemap).toBe(false);
  });

  it("uses the config's sourcemap when no option is given", async () => {
    let seen: BuildOptions | undefined;
    const bundler: Bundler = {
      watch: (): Promise<Watcher> => Promise.reject(new Error("not used by build()")),
      build: (options) => {
        seen = options;
        return Promise.resolve({ outDir: options.outDir, entryFile: "bundle.js", files: [] });
      },
    };

    await build({
      cwd: "/app",
      entry: "unused",
      bundler,
      config: { outDir: "/app/dist", entry: undefined, sourcemap: true },
    });

    expect(seen?.sourcemap).toBe(true);
  });

  it("propagates a bundler build failure", async () => {
    const bundler = makeFakeBundler(() => Promise.reject(new Error("syntax error")));

    await expect(build({ cwd: "/app", entry: "unused", bundler })).rejects.toThrow("syntax error");
  });
});
