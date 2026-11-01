// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * Bundles one test file with rolldown and runs it in a QuickJS context.
 * Usage: `node run.mts [--config <path>] <file>`.
 */

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import variant from "@jitl/quickjs-ng-wasmfile-release-sync";
import { newQuickJSWASMModule } from "quickjs-emscripten";
import { rolldown } from "rolldown";
import dedent from "ts-dedent";
import type { Plugin } from "rolldown";

interface Config {
  plugins?: Plugin[];
  stubs?: string[];
}

const usage = "usage: run.mts [--config <path>] <file>";
const entryId = "\0quickjs-entry";
const vitestId = "\0quickjs-vitest";
/** The globals that vitest reads while it loads. */
const stubsPath = resolve(import.meta.dirname, "stubs.js");
const runnerPath = resolve(import.meta.dirname, "runner.mts");

/** Returns the parsed options, or the message to print when the arguments are wrong. */
function parse(argv: string[]): { config?: string; file: string } | string {
  let config: string | undefined;
  let file: string | undefined;
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]!;
    if (arg === "--config") {
      config = argv[++i];
      if (config === undefined) return usage;
    } else if (arg.startsWith("-")) return usage;
    else file = arg;
  }
  return file === undefined ? "missing test file" : { config, file };
}

function describeError(error: { name?: string; message?: string }): string {
  return `${error.name}: ${error.message}`;
}

async function bundle(file: string, plugins: Plugin[]): Promise<string> {
  const build = await rolldown({
    input: entryId,
    platform: "neutral",
    external: [/^node:/],
    // the timing warning is noise for a one-off bundle
    checks: { bundlerTimings: false },
    resolve: { conditionNames: ["import", "default"], mainFields: ["module", "main"] },
    plugins: [
      {
        name: "quickjs-virtual",
        async resolveId(id, importer) {
          if (id === entryId) return id;
          // the sideEffects list of the package leaves out test files, which export nothing
          if (id === file) return { id, moduleSideEffects: true };
          if (id !== "vitest") return null;
          if (importer === vitestId) return this.resolve(id, runnerPath, { skipSelf: true });
          return vitestId;
        },
        load(id) {
          if (id === vitestId) {
            return dedent`
              export { expect, vi } from "vitest";
              export { describe, it, beforeEach, afterEach } from ${JSON.stringify(runnerPath)};
            `;
          }
          if (id !== entryId) return null;
          return dedent`
            import { run } from ${JSON.stringify(runnerPath)};
            import ${JSON.stringify(file)};
            globalThis.__failed = run();
          `;
        },
      },
      ...plugins,
    ],
  });
  const { output } = await build.generate({ format: "iife" });
  await build.close();
  return output[0].code;
}

async function main(): Promise<number> {
  const args = parse(process.argv.slice(2));
  if (typeof args === "string") {
    console.error(args);
    return 2;
  }
  const file = resolve(args.file);
  try {
    await readFile(file);
  } catch {
    console.error(`cannot read ${args.file}`);
    return 2;
  }
  const config: Config = args.config
    ? ((await import(pathToFileURL(resolve(args.config)).href)) as { default: Config }).default
    : {};

  let code: string;
  try {
    code = await bundle(file, config.plugins ?? []);
  } catch (error) {
    console.error((error as Error).message);
    return 1;
  }
  const module = await newQuickJSWASMModule(variant);
  const runtime = module.newRuntime();
  const vm = runtime.newContext();
  let exit = 0;
  const fail = (error: { name?: string; message?: string }): void => {
    console.error(describeError(error));
    const missing = /^(\S+) is not defined/.exec(error.message ?? "");
    if (error.name === "ReferenceError" && missing) {
      console.error(`the QuickJS context has no global ${missing[1]}`);
    }
    exit = 1;
  };
  try {
    const log = vm.newFunction("__log", (text) => console.log(vm.getString(text)));
    vm.setProp(vm.global, "__log", log);
    log.dispose();
    for (const source of [await readFile(stubsPath, "utf8"), ...(config.stubs ?? []), code]) {
      const result = vm.evalCode(source, "bundle.js", { strict: true });
      if (result.error) {
        fail(vm.dump(result.error));
        result.error.dispose();
        break;
      }
      result.value.dispose();
    }
    while (exit === 0 && runtime.hasPendingJob()) {
      const result = runtime.executePendingJobs();
      if (result.error) {
        fail(vm.dump(result.error));
        result.error.dispose();
      }
    }
    if (exit === 0) {
      const failed = vm.getProp(vm.global, "__failed");
      if (vm.dump(failed) !== 0) exit = 1;
      failed.dispose();
    }
  } finally {
    vm.dispose();
    runtime.dispose();
  }
  return exit;
}

process.exitCode = await main();
