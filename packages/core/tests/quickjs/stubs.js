// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * The globals that vitest reads while it loads, evaluated in the QuickJS context.
 *
 * After a vitest upgrade, run `pnpm -F incajs test:quickjs src/interfaces.test.mts`.
 * The line `the QuickJS context has no global <name>` names a global to stub here.
 * A stub for core code goes in the config file passed with `--config`.
 */

globalThis.process = {
  env: {},
  argv: [],
  platform: "quickjs",
  versions: {},
  cwd: () => "/",
  nextTick: (f) => Promise.resolve().then(f),
  emitWarning() {},
};
globalThis.EventTarget = class {
  addEventListener() {}
  removeEventListener() {}
  dispatchEvent() {}
};
globalThis.Event = class {};
globalThis.console = Object.fromEntries(
  ["log", "info", "debug", "warn", "error"].map((n) => [n, (...a) => __log(a.join(" "))]),
);
