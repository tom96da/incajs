// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { build } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import * as core from "../../adapterCore.mts";
import { resolveViteConfig } from "./config.mts";
import type { CapturedOutput } from "./captureOutput.mts";
import type { ResolveConfigOptions } from "./config.mts";

function optionsFor(mode: ResolveConfigOptions["mode"]): ResolveConfigOptions {
  return {
    core,
    runtimeModuleName: "runtime",
    entry: "/unused/src/main.ts",
    outDir: "/unused/dist",
    mode,
    watch: false,
    onOutput: () => {},
    stdout: process.stdout,
    stderr: process.stderr,
    quiet: true,
  };
}

describe("resolveViteConfig source maps", () => {
  it("emits a sidecar map with paths only in development", () => {
    const config = resolveViteConfig(optionsFor("development"));

    expect(config.build?.sourcemap).toBe(true);
    expect(config.build?.rolldownOptions?.output).toMatchObject({ sourcemapExcludeSources: true });
  });

  it("leaves source maps off in production", () => {
    const config = resolveViteConfig(optionsFor("production"));

    expect(config.build?.sourcemap).toBe(false);
  });
});

describe("development library build", () => {
  let scratch: string;

  beforeAll(async () => {
    scratch = await mkdtemp(path.join(tmpdir(), "inca-sourcemap-"));
  });

  afterAll(async () => {
    await rm(scratch, { recursive: true, force: true });
  });

  it("writes bundle.js.map that names the source and omits sourcesContent, reports it as output, and links it from bundle.js", async () => {
    const srcDir = path.join(scratch, "app");
    await mkdir(srcDir);
    const entry = path.join(srcDir, "main.ts");
    await writeFile(
      entry,
      'export const greet = (name: string): string => `hello ${name}`;\nconsole.log(greet("inca"));\n',
    );
    const outDir = path.join(scratch, "dist");

    const outputs: CapturedOutput[] = [];
    await build(
      resolveViteConfig({
        ...optionsFor("development"),
        entry,
        outDir,
        onOutput: (output) => outputs.push(output),
      }),
    );

    expect(outputs.at(-1)?.files).toContain("bundle.js.map");

    const map = JSON.parse(await readFile(path.join(outDir, "bundle.js.map"), "utf8")) as {
      sources: string[];
      sourcesContent?: unknown;
    };
    expect(map.sources.some((source) => source.endsWith("main.ts"))).toBe(true);
    expect(map.sourcesContent).toBeUndefined();

    const js = await readFile(path.join(outDir, "bundle.js"), "utf8");
    expect(js.trimEnd().endsWith("//# sourceMappingURL=bundle.js.map")).toBe(true);
  });
});
