// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * A running app's own `globalThis.__inca_dev__`, installed by the host
 * under `--dev`.
 */
interface IncaDev {
  send: (method: string, paramsJson: string) => void;
  receive?: (method: string, paramsJson: string) => void;
  setLoadFailed?: (failed: boolean) => void;
  /**
   * Builds an async function from `body` that takes `params` in order. Its
   * stack frames name `filename`, and a frame on line N of `body` reports
   * line N + 1.
   * @param params - the parameter names, in order
   * @param body - the function body source
   * @param filename - the file name that stack frames report
   * @returns the async function, taking one argument per name in `params`
   * @throws `TypeError` when any argument holds a NUL byte
   * @throws `SyntaxError` when `body` does not parse, its stack naming `filename` and the line
   */
  compile?: (
    params: string[],
    body: string,
    filename: string,
  ) => (...args: unknown[]) => Promise<unknown>;
}

declare global {
  // eslint-disable-next-line no-var
  var __inca_dev__: IncaDev | undefined;
}
