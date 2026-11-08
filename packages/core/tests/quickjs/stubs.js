// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * The globals that vitest reads while it loads, evaluated in the QuickJS context.
 *
 * After a vitest upgrade, run `pnpm -F incajs test:quickjs src/document.test.mts`.
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

// A virtual clock: `__nextTimer()` fires the earliest due timer, ties by start order.
const timers = new Map();
let now = 0;
let started = 0;
const start =
  (repeat) =>
  (callback, delay = 0, ...args) => {
    const wait = Math.max(0, Number(delay) || 0);
    const id = ++started;
    timers.set(id, { callback, args, wait, repeat, at: now + wait });
    return id;
  };
globalThis.setTimeout = start(false);
globalThis.setInterval = start(true);
globalThis.clearTimeout = globalThis.clearInterval = (id) => void timers.delete(id);
// Limit: an awaited rejection stays unhandled and an async function rejection is not tracked.
const listeners = new Set();
process.on = (name, listener) => {
  if (name === "unhandledRejection") listeners.add(listener);
  return process;
};
process.off = (name, listener) => {
  if (name === "unhandledRejection") listeners.delete(listener);
  return process;
};
const handled = new WeakSet();
const rejected = new Map();
const nativeThen = Reflect.get(Promise.prototype, "then");
const nativeReject = Reflect.get(Promise, "reject");
const watch = (promise) =>
  void nativeThen.call(promise, undefined, (reason) => {
    if (!handled.has(promise)) rejected.set(promise, reason);
  });
Reflect.set(Promise.prototype, "then", function (onFulfilled, onRejected) {
  handled.add(this);
  rejected.delete(this);
  const next = nativeThen.call(this, onFulfilled, onRejected);
  watch(next);
  return next;
});
Promise.reject = function (reason) {
  const promise = nativeReject.call(this, reason);
  watch(promise);
  return promise;
};
globalThis.__nextTimer = () => {
  const reasons = [...rejected.values()];
  rejected.clear();
  for (const reason of reasons) for (const listener of listeners) listener(reason);
  let next;
  for (const [id, timer] of timers) if (!next || timer.at < next[1].at) next = [id, timer];
  if (!next) return false;
  const [id, timer] = next;
  now = timer.at;
  if (timer.repeat) timer.at = now + timer.wait;
  else timers.delete(id);
  timer.callback(...timer.args);
  return true;
};
