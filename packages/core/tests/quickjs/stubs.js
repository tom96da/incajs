// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The globals that vitest reads while it loads, evaluated in the QuickJS context.
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
