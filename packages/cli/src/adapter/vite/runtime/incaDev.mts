// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * A running app's own `globalThis.__inca_dev__`, installed by the host
 * under `--dev`.
 */
export interface IncaDev {
  send: (method: string, paramsJson: string) => void;
  receive?: (method: string, paramsJson: string) => void;
  setLoadFailed?: (failed: boolean) => void;
  /**
   * Builds an async function from `body` that takes `params` in order. Its
   * stack frames name `filename`, and a frame on line N of `body` reports
   * line N + 1. A NUL byte in any argument is rejected, and a syntax error
   * raises a `SyntaxError` that names `filename`.
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
