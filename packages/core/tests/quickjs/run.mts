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
import { hoistMocks } from "@vitest/mocker/node";
import { newQuickJSWASMModule } from "quickjs-emscripten";
import { rolldown } from "rolldown";
import dedent from "ts-dedent";
import type { QuickJSContext } from "quickjs-emscripten";
import type { Plugin, PluginContext } from "rolldown";

interface Config {
  plugins?: Plugin[];
  stubs?: string[];
  /** Specifiers a test loads with a dynamic `import()` that rolldown cannot see. */
  modules?: string[];
}

interface Bundle {
  /** The chunk of the virtual entry. */
  entry: string;
  /** Chunk file name to its code. */
  code: Map<string, string>;
  /** Chunk file name to the module it holds and that module's exports. */
  modules: Map<string, { id: string; exports: string[] }>;
  /** Extra specifier to its chunk file name. */
  extras: Map<string, string>;
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

function describeError(error: { name?: string; message?: string; stack?: string }): string {
  return `${error.name}: ${error.message}${error.stack ? `\n${error.stack}` : ""}`;
}

const mockCall = /\bvi\s*\.\s*(?:mock|unmock|doMock|doUnmock)\s*\(\s*(["'])([^"']+)\1/g;

/**
 * Rewrites the specifier of every mock call to the id it resolves to, then moves the mock calls
 * above the imports.
 */
async function hoist(
  ctx: Pick<PluginContext, "resolve" | "parse">,
  code: string,
  file: string,
): Promise<string> {
  let out = "";
  let last = 0;
  for (const match of code.matchAll(mockCall)) {
    const resolved = await ctx.resolve(match[2]!, file);
    const at = match.index + match[0].lastIndexOf(match[2]!);
    out += code.slice(last, at) + (resolved?.id ?? match[2]!);
    last = at + match[2]!.length;
  }
  out += code.slice(last);
  return hoistMocks(out, file, (c) => ctx.parse(c, { lang: "ts" }) as never)?.code ?? out;
}

async function bundle(file: string, config: Config): Promise<Bundle> {
  const extraIds = new Map<string, string>();
  const build = await rolldown({
    input: { entry: entryId, ...Object.fromEntries((config.modules ?? []).map((m) => [m, m])) },
    platform: "neutral",
    // the timing warning is noise for a one-off bundle
    checks: { bundlerTimings: false },
    resolve: { conditionNames: ["import", "default"], mainFields: ["module", "main"] },
    plugins: [
      {
        name: "quickjs-virtual",
        async resolveId(id, importer, options) {
          if (id === entryId) return id;
          // the sideEffects list of the package leaves out test files, which export nothing
          if (id === file) return { id, moduleSideEffects: true };
          if (!importer && config.modules?.includes(id)) {
            const resolved = await this.resolve(id, path.resolve("quickjs-extra"), {
              ...options,
              skipSelf: true,
            });
            if (resolved) extraIds.set(resolved.id, id);
            return resolved;
          }
          if (id !== "vitest") return null;
          if (importer === vitestId) return this.resolve(id, runnerPath, { skipSelf: true });
          return vitestId;
        },
        async transform(code, id) {
          if (id !== file) return null;
          return await hoist(this, code, id);
        },
        load(id) {
          if (id === vitestId) {
            return dedent`
              import { expect, expectTypeOf, vi } from "vitest";
              import { describe, it, beforeEach, afterEach } from ${JSON.stringify(runnerPath)};
              import { withModuleMocks } from ${JSON.stringify(runnerPath)};
              const wrapped = withModuleMocks(vi);
              export { expect, expectTypeOf, describe, it, beforeEach, afterEach, wrapped as vi };
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
      ...(config.plugins ?? []),
    ],
  });
  const { output } = await build.generate({ format: "esm", preserveModules: true });
  await build.close();
  const result: Bundle = { entry: "", code: new Map(), modules: new Map(), extras: new Map() };
  for (const chunk of output) {
    if (chunk.type !== "chunk") continue;
    result.code.set(chunk.fileName, chunk.code);
    const id = chunk.facadeModuleId ?? "";
    result.modules.set(chunk.fileName, { id, exports: chunk.exports });
    if (id === entryId) result.entry = chunk.fileName;
    const extra = extraIds.get(id);
    if (extra) result.extras.set(extra, chunk.fileName);
  }
  return result;
}

/**
 * The module that replaces a mocked one: it re-exports the names of the factory result. A
 * promise result needs a top-level await, which a module without one avoids.
 */
function mockSource(key: number, async: boolean, exports: string[]): string {
  const lines = [`const m = ${async ? "await " : ""}globalThis.__quickjs_values.get(${key});`];
  exports.forEach((name, i) => {
    // a CommonJS module has its names on `default`
    lines.push(`const v${i} = m[${JSON.stringify(name)}] ?? m.default?.[${JSON.stringify(name)}];`);
    lines.push(`export { v${i} as ${JSON.stringify(name)} };`);
  });
  return lines.join("\n");
}

interface Counts {
  passed: number;
  failed: number;
}

const generation = /#\d+$/;

/** Calls a global function of the context and returns its result, or the error. */
function callGlobal(
  vm: QuickJSContext,
  name: string,
  ...args: string[]
): { value?: unknown; error?: unknown } {
  const fn = vm.getProp(vm.global, name);
  const handles = args.map((arg) => vm.newString(arg));
  const result = vm.callFunction(fn, vm.undefined, ...handles);
  for (const handle of [fn, ...handles]) handle.dispose();
  if (result.error) {
    const error = vm.dump(result.error);
    result.error.dispose();
    return { error };
  }
  const value = vm.dump(result.value);
  result.value.dispose();
  return { value };
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
  let built: Bundle;
  try {
    built = await bundle(path.resolve(file), config);
  } catch (error) {
    console.error((error as Error).message);
    return crashed;
  }
  const module = await newQuickJSWASMModule(variant);
  const runtime = module.newRuntime();
  const vm = runtime.newContext();
  let counts: Counts | undefined;
  let broken = false;
  let current = 0;
  const mocked = new Set<string>();
  const fail = (error: { name?: string; message?: string; stack?: string }): void => {
    console.error(describeError(error));
    const missing = /^(\S+) is not defined/.exec(error.message ?? "");
    if (error.name === "ReferenceError" && missing) {
      console.error(`the QuickJS context has no global ${missing[1]}`);
    }
    broken = true;
  };
  let mocks = 0;
  runtime.setModuleLoader(
    (name, context) => {
      const chunk = name.replace(generation, "");
      const info = built.modules.get(chunk);
      if (info && mocked.has(info.id)) {
        const key = ++mocks;
        const made = callGlobal(context, "__quickjs_make_mock", info.id, String(key));
        if (made.error) return { error: new Error(describeError(made.error as Error)) };
        return mockSource(key, made.value === "promise", info.exports);
      }
      return built.code.get(chunk) ?? { error: new Error(`cannot find module ${chunk}`) };
    },
    (base, requested) => {
      const target = requested.startsWith(".")
        ? path.posix.join(path.posix.dirname(base.replace(generation, "")), requested)
        : (built.extras.get(requested) ?? requested);
      return `${target}#${current}`;
    },
  );
  // no dispose: the engine asserts at disposal after a collected cycle that held a weak reference
  const define = (name: string, impl: (...args: string[]) => void): void => {
    const fn = vm.newFunction(name, (...handles) => impl(...handles.map((h) => vm.getString(h))));
    vm.setProp(vm.global, name, fn);
    fn.dispose();
  };
  define("__log", (text) => {
    const summary = /^(\d+) passed, (\d+) failed$/.exec(text!);
    if (summary) counts = { passed: Number(summary[1]), failed: Number(summary[2]) };
    if (verbose || summary || text!.startsWith("FAIL ")) console.log(text);
  });
  define("__reset_modules", () => void current++);
  define("__mock", (id, on) => {
    if (on === "1") mocked.add(id!);
    else mocked.delete(id!);
  });
  for (const source of [stubs, ...(config.stubs ?? [])]) {
    const result = vm.evalCode(source, "bundle.js", { strict: true });
    if (result.error) {
      fail(vm.dump(result.error));
      result.error.dispose();
      break;
    }
    result.value.dispose();
  }
  const start = broken
    ? undefined
    : vm.evalCode(built.code.get(built.entry)!, `${built.entry}#${current}`, { type: "module" });
  if (start?.error) {
    fail(vm.dump(start.error));
    start.error.dispose();
  }
  for (;;) {
    while (!broken && runtime.hasPendingJob()) {
      const result = runtime.executePendingJobs();
      if (result.error) {
        fail(vm.dump(result.error));
        result.error.dispose();
      }
    }
    if (broken || counts) break;
    const next = callGlobal(vm, "__nextTimer");
    if (next.error) fail(next.error as Error);
    else if (!next.value) break;
  }
  if (start && !start.error) {
    const state = vm.getPromiseState(start.value);
    if (state.type === "rejected") {
      fail(vm.dump(state.error));
      state.error.dispose();
    } else if (state.type === "fulfilled" && !state.notAPromise) state.value.dispose();
    start.value.dispose();
  }
  if (!broken && !counts) {
    const handle = vm.getProp(vm.global, "__current_test");
    const name = vm.dump(handle);
    handle.dispose();
    console.log(`FAIL ${name}: the test run stopped waiting`);
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
