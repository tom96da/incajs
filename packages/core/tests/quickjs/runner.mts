// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/** The `describe`, `it` and hook shim that runs inside the QuickJS context. */

declare function __log(text: string): void;

type Body = () => unknown;
interface Test {
  name: string;
  body: Body;
}

const tests: Test[] = [];
const before: Body[] = [];
const after: Body[] = [];
const names: string[] = [];

export function describe(name: string, body: Body): void {
  names.push(name);
  body();
  names.pop();
}

export const it = Object.assign(
  (name: string, body: Body): void => {
    tests.push({ name: [...names, name].join(" > "), body });
  },
  {
    each:
      (rows: unknown[]) =>
      (name: string, body: (...row: any[]) => unknown): void => {
        for (const row of rows) {
          const args = Array.isArray(row) ? row : [row];
          it(name.replace("%s", String(args[0])), () => body(...args));
        }
      },
  },
);

export const beforeEach = (hook: Body): void => void before.push(hook);
export const afterEach = (hook: Body): void => void after.push(hook);

function runOne(test: Test): void {
  try {
    for (const hook of before) hook();
    if (isThenable(test.body())) throw new Error("async test bodies are not supported");
  } finally {
    for (const hook of after) hook();
  }
}

function isThenable(value: unknown): boolean {
  return typeof (value as { then?: unknown } | null)?.then === "function";
}

/** Runs the collected tests and returns the number of failures. */
export function run(): number {
  let failed = 0;
  for (const test of tests) {
    try {
      runOne(test);
      __log(`ok   ${test.name}`);
    } catch (error) {
      failed++;
      __log(`FAIL ${test.name}: ${(error as Error).message}`);
    }
  }
  __log(`${tests.length - failed} passed, ${failed} failed`);
  return failed;
}
