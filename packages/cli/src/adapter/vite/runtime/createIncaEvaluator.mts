// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  ESModulesEvaluator,
  type EvaluatedModuleNode,
  type ModuleEvaluator,
  type ModuleRunnerContext,
  ssrDynamicImportKey,
  ssrExportAllKey,
  ssrExportNameKey,
  ssrImportKey,
  ssrImportMetaKey,
  ssrModuleExportsKey,
} from "vite/module-runner";

// The cast gives the base class the `ModuleEvaluator` signature, which carries the module.
/** Runs each module through `__inca_dev__.compile`. */
class IncaEvaluator extends (ESModulesEvaluator as new () => ModuleEvaluator) {
  // Offsets Vite's source map lookup by the header line `compile` adds.
  override readonly startOffset = 1;

  override async runInlinedModule(
    context: ModuleRunnerContext,
    code: string,
    module: Readonly<EvaluatedModuleNode>,
  ): Promise<void> {
    const dev = globalThis.__inca_dev__;
    if (!dev?.compile) {
      throw new Error("__inca_dev__.compile is not installed");
    }
    // Same parameter order as the stock evaluator.
    const keys: (keyof ModuleRunnerContext)[] = [
      ssrModuleExportsKey,
      ssrImportMetaKey,
      ssrImportKey,
      ssrDynamicImportKey,
      ssrExportAllKey,
      ssrExportNameKey,
    ];
    // `compile` takes the place of `new AsyncFunction`, so a frame names the module.
    // It rejects a NUL byte, so each one is written as `\u0000`, which keeps its value in a
    // literal. A virtual module's id starts with one.
    const initModule = dev.compile(
      keys,
      code.replaceAll("\0", "\\u0000"),
      module.id.replaceAll("\0", "\\u0000"),
    );

    await initModule(...keys.map((key) => context[key]));

    Object.seal(context[ssrModuleExportsKey]);
  }
}

/**
 * Builds a `ModuleEvaluator` that runs each HMR module through the host's
 * `__inca_dev__.compile`. A stack frame of a module names the module's id,
 * with each NUL byte written as `\u0000`.
 * Running a module rejects when `globalThis.__inca_dev__` or its `compile`
 * is missing.
 * @returns the evaluator for a `ModuleRunner`
 */
export function createIncaEvaluator(): ModuleEvaluator {
  return new IncaEvaluator();
}
