// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/** The `describe`, `it` and hook shim that runs inside the QuickJS context. */

declare function __log(text: string): void;

type Body = () => unknown;
type Factory = () => unknown;
interface Suite {
  parent?: Suite;
  before: Body[];
  after: Body[];
}
interface Test {
  name: string;
  body: Body;
  suite: Suite;
}

declare function __reset_modules(): void;
declare function __mock(id: string, on: "1" | "0"): void;

const tests: Test[] = [];
const names: string[] = [];
const root: Suite = { before: [], after: [] };
let suite = root;

export function describe(name: string, body: Body): void {
  names.push(name);
  suite = { parent: suite, before: [], after: [] };
  body();
  suite = suite.parent!;
  names.pop();
}

export const it = Object.assign(
  (name: string, body: Body): void => {
    tests.push({ name: [...names, name].join(" > "), body, suite });
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

export const beforeEach = (hook: Body): void => void suite.before.push(hook);
export const afterEach = (hook: Body): void => void suite.after.push(hook);

function isThenable(value: unknown): boolean {
  return typeof (value as { then?: unknown } | null)?.then === "function";
}

/** The suites of a test from the outermost to the innermost. */
function chain(test: Test): Suite[] {
  const suites: Suite[] = [];
  for (let s: Suite | undefined = test.suite; s; s = s.parent) suites.unshift(s);
  return suites;
}

async function runOne(test: Test): Promise<void> {
  const suites = chain(test);
  try {
    for (const s of suites) for (const hook of s.before) await hook();
    await test.body();
  } finally {
    for (const s of suites.toReversed()) for (const hook of s.after) await hook();
  }
}

/** Runs the collected tests and returns the number of failures. */
export async function run(): Promise<number> {
  let failed = 0;
  for (const test of tests) {
    (globalThis as { __current_test?: string }).__current_test = test.name;
    try {
      await runOne(test);
      __log(`ok   ${test.name}`);
    } catch (error) {
      failed++;
      __log(`FAIL ${test.name}: ${(error as Error).message}`);
    }
  }
  __log(`${tests.length - failed} passed, ${failed} failed`);
  return failed;
}

/**
 * Returns `vi` with the module members replaced: a mocked module is served by the run tool, which
 * evaluates a module again after `resetModules`.
 */
export function withModuleMocks<T extends object>(vi: T): T {
  const factories = new Map<string, Factory>();
  const values = new Map<number, unknown>();
  Object.assign(globalThis, {
    __quickjs_values: values,
    // the run tool calls this when it loads a module that a factory replaces
    __quickjs_make_mock(id: string, key: string): string {
      const value = factories.get(id)!();
      values.set(Number(key), value);
      return isThenable(value) ? "promise" : "value";
    },
  });
  const mock = (id: string, factory?: Factory): void => {
    if (!factory) throw new Error(`vi.mock("${id}") needs a factory in the QuickJS run`);
    factories.set(id, factory);
    __mock(id, "1");
  };
  const unmock = (id: string): void => {
    factories.delete(id);
    __mock(id, "0");
  };
  return Object.assign(Object.create(vi) as T, {
    resetModules() {
      __reset_modules();
      return this;
    },
    mock,
    doMock: mock,
    unmock,
    doUnmock: unmock,
    hoisted: <R,>(fn: () => R): R => fn(),
  });
}
