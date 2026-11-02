// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * Bundles test files with rolldown and runs each in its own QuickJS context.
 * Without a file argument it runs every `*.test.mts` below the working directory.
 * Usage: `node run.mts [--config <path>] [<file>...]`.
 */

import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
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

const usage = "usage: run.mts [--config <path>] [<file>...]";
const entryId = "\0quickjs-entry";
const vitestId = "\0quickjs-vitest";
/** The file with the globals that vitest reads while it loads. */
const stubsPath = path.resolve(import.meta.dirname, "stubs.js");
const runnerPath = path.resolve(import.meta.dirname, "runner.mts");

/** Returns the parsed options, or the usage line when the arguments are wrong. */
function parse(argv: string[]): { config?: string; files: string[] } | string {
  let config: string | undefined;
  const files: string[] = [];
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]!;
    if (arg === "--config") {
      config = argv[++i];
      if (config === undefined) return usage;
    } else if (arg.startsWith("-")) return usage;
    else files.push(arg);
  }
  return { config, files };
}

/** Lists the test files below the working directory, sorted. */
async function scan(): Promise<string[]> {
  const entries = await readdir(process.cwd(), { recursive: true });
  const skipped = (part: string): boolean =>
    part === "node_modules" || part === "dist" || part.startsWith(".");
  return entries
    .filter((entry) => entry.endsWith(".test.mts") && !entry.split(path.sep).some(skipped))
    .sort();
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

interface Counts {
  passed: number;
  failed: number;
}

/**
 * Runs one test file in a fresh context. Prints every line of the file when `verbose`, and the
 * FAIL lines and the summary otherwise.
 */
async function runFile(
  file: string,
  config: Config,
  stubs: string,
  verbose: boolean,
): Promise<Counts> {
  const crashed = { passed: 0, failed: 1 };
  let code: string;
  try {
    code = await bundle(path.resolve(file), config.plugins ?? []);
  } catch (error) {
    console.error((error as Error).message);
    return crashed;
  }
  const module = await newQuickJSWASMModule(variant);
  const runtime = module.newRuntime();
  const vm = runtime.newContext();
  let counts: Counts | undefined;
  let broken = false;
  const fail = (error: { name?: string; message?: string }): void => {
    console.error(describeError(error));
    const missing = /^(\S+) is not defined/.exec(error.message ?? "");
    if (error.name === "ReferenceError" && missing) {
      console.error(`the QuickJS context has no global ${missing[1]}`);
    }
    broken = true;
  };
  try {
    const log = vm.newFunction("__log", (handle) => {
      const text = vm.getString(handle);
      const summary = /^(\d+) passed, (\d+) failed$/.exec(text);
      if (summary) counts = { passed: Number(summary[1]), failed: Number(summary[2]) };
      if (verbose || summary || text.startsWith("FAIL ")) console.log(text);
    });
    vm.setProp(vm.global, "__log", log);
    log.dispose();
    for (const source of [stubs, ...(config.stubs ?? []), code]) {
      const result = vm.evalCode(source, "bundle.js", { strict: true });
      if (result.error) {
        fail(vm.dump(result.error));
        result.error.dispose();
        break;
      }
      result.value.dispose();
    }
    while (!broken && runtime.hasPendingJob()) {
      const result = runtime.executePendingJobs();
      if (result.error) {
        fail(vm.dump(result.error));
        result.error.dispose();
      }
    }
  } finally {
    vm.dispose();
    runtime.dispose();
  }
  if (broken || !counts) return crashed;
  return counts;
}

async function main(): Promise<number> {
  const args = parse(process.argv.slice(2));
  if (typeof args === "string") {
    console.error(args);
    return 2;
  }
  const files = args.files.length > 0 ? args.files : await scan();
  if (files.length === 0) {
    console.error("no test file found");
    return 2;
  }
  for (const file of files) {
    try {
      await readFile(file);
    } catch {
      console.error(`cannot read ${file}`);
      return 2;
    }
  }
  const config: Config = args.config
    ? ((await import(pathToFileURL(path.resolve(args.config)).href)) as { default: Config }).default
    : {};
  const stubs = await readFile(stubsPath, "utf8");

  const total = { passed: 0, failed: 0 };
  for (const file of files) {
    if (files.length > 1) console.log(`== ${file}`);
    const counts = await runFile(file, config, stubs, files.length === 1);
    total.passed += counts.passed;
    total.failed += counts.failed;
  }
  if (files.length > 1) {
    console.log(`${files.length} files, ${total.passed} passed, ${total.failed} failed`);
  }
  return total.failed > 0 ? 1 : 0;
}

process.exitCode = await main();
