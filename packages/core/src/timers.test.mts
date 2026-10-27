// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, expect, it, vi } from "vitest";

/** Node's timers, put back after each test. */
const nodeTimers = { setTimeout, setInterval, clearTimeout, clearInterval };
/** The globals as the module defines them, without Node's overloads. */
const g = globalThis as unknown as {
  setTimeout(handler: unknown, delay?: unknown, ...args: unknown[]): number;
  setInterval(handler: unknown, delay?: unknown, ...args: unknown[]): number;
  clearTimeout(id?: unknown): void;
  clearInterval(id?: unknown): void;
  __inca_timer__: (id: number) => void;
};
/** The id the mocked host returns next. */
let nextId = 1;
const startTimer = vi.fn<(delay: number, repeat: boolean) => number | undefined>(() => nextId++);
const stopTimer = vi.fn<(id: number) => void>();

/**
 * Loads a fresh copy of the module over `native`.
 * @param native - what `globalThis.__inca_native__` holds while it loads
 */
async function load(native: object): Promise<void> {
  globalThis.__inca_native__ = native as never;
  vi.resetModules();
  await import("./timers.mts");
}

beforeEach(async () => {
  nextId = 1;
  startTimer.mockClear();
  stopTimer.mockClear();
  await load({ startTimer, stopTimer });
});

afterEach(() => {
  Object.assign(globalThis, nodeTimers);
  delete (globalThis as { __inca_timer__?: unknown }).__inca_timer__;
});

it("returns the id the host gives", () => {
  startTimer.mockReturnValueOnce(7);

  expect(g.setTimeout(() => {})).toBe(7);
});

it.each([
  ["setTimeout", false],
  ["setInterval", true],
] as const)("%s queues its timer with repeat %s", (name, repeat) => {
  g[name](() => {}, 100);

  expect(startTimer).toHaveBeenCalledWith(100, repeat);
});

it.each([
  ["negative", -5, 0],
  ["string", "5", 5],
  ["wrapped", 2 ** 32 + 10, 10],
])("reads a %s delay as whole milliseconds", (_, delay, ms) => {
  g.setTimeout(() => {}, delay);

  expect(startTimer).toHaveBeenCalledWith(ms, false);
});

it.each(["setTimeout", "setInterval"] as const)(
  "%s throws a TypeError for a handler that is not a function",
  (name) => {
    expect(() => g[name]("1")).toThrow(
      new TypeError(`${name} takes a function. pass a function as the handler`),
    );
    expect(startTimer).not.toHaveBeenCalled();
  },
);

it("throws a RangeError once the host has no id left", () => {
  startTimer.mockReturnValueOnce(undefined);

  expect(() => g.setTimeout(() => {})).toThrow(
    new RangeError("timer ids are exhausted. restart the app"),
  );
});

it("passes the extra arguments to the handler", () => {
  const handler = vi.fn<(a: unknown, b: unknown) => void>();
  const id = g.setTimeout(handler, 0, "x", 2);

  g.__inca_timer__(id);

  expect(handler).toHaveBeenCalledWith("x", 2);
});

it("calls the handler with globalThis as this", () => {
  const handler = vi.fn<() => void>();

  g.__inca_timer__(g.setTimeout(handler));

  expect(handler.mock.contexts[0]).toBe(globalThis);
});

it("runs a one-shot handler once", () => {
  const handler = vi.fn<() => void>();
  const id = g.setTimeout(handler);

  g.__inca_timer__(id);
  g.__inca_timer__(id);

  expect(handler).toHaveBeenCalledTimes(1);
});

it("keeps a repeating handler after it runs", () => {
  const handler = vi.fn<() => void>();
  const id = g.setInterval(handler);

  g.__inca_timer__(id);
  g.__inca_timer__(id);

  expect(handler).toHaveBeenCalledTimes(2);
});

it("throws a one-shot handler's error out of the hook once", () => {
  const id = g.setTimeout(() => {
    throw new Error("boom");
  });

  expect(() => g.__inca_timer__(id)).toThrow("boom");
  expect(() => g.__inca_timer__(id)).not.toThrow();
});

it("stops a timer of an earlier load of core", async () => {
  const handler = vi.fn<() => void>();
  const id = g.setInterval(handler);

  await load({ startTimer, stopTimer });
  g.__inca_timer__(id);

  expect(stopTimer).toHaveBeenCalledWith(id);
  expect(handler).not.toHaveBeenCalled();
});

it("stops the host timer on clear", () => {
  const id = g.setTimeout(() => {});

  g.clearTimeout(id);

  expect(stopTimer).toHaveBeenCalledWith(id);
});

it("does not run a cleared handler", () => {
  const handler = vi.fn<() => void>();
  const id = g.setTimeout(handler);
  g.clearTimeout(id);

  g.__inca_timer__(id);

  expect(handler).not.toHaveBeenCalled();
});

it("clears a timeout with clearInterval", () => {
  const id = g.setTimeout(() => {});

  g.clearInterval(id);

  expect(stopTimer).toHaveBeenCalledWith(id);
});

it.each([
  ["string", "1"],
  ["fractional", 1.9],
])("reads a %s id of clear as a 32-bit integer", (_, id) => {
  g.setTimeout(() => {});

  g.clearTimeout(id);

  expect(stopTimer).toHaveBeenCalledWith(1);
});

it("ignores an id no timer holds in clear", () => {
  g.clearTimeout(99);

  expect(stopTimer).not.toHaveBeenCalled();
});

it.each([
  ["setTimeout", 1],
  ["setInterval", 1],
  ["clearTimeout", 0],
  ["clearInterval", 0],
] as const)("gives %s the length %i of the Web function", (name, length) => {
  expect(g[name].length).toBe(length);
});

it("keeps the hook when a script assigns to it", () => {
  const hook = g.__inca_timer__;

  expect(() => {
    g.__inca_timer__ = () => {};
  }).toThrow(TypeError);
  expect(g.__inca_timer__).toBe(hook);
});

it("is the latest copy's hook once core loads again", async () => {
  const hook = g.__inca_timer__;

  await load({ startTimer, stopTimer });

  expect(g.__inca_timer__).not.toBe(hook);
});

it("leaves Node's four timer functions alone without the host's timer natives", async () => {
  Object.assign(globalThis, nodeTimers);

  await load({});

  expect(globalThis.setTimeout).toBe(nodeTimers.setTimeout);
  expect(globalThis.setInterval).toBe(nodeTimers.setInterval);
  expect(globalThis.clearTimeout).toBe(nodeTimers.clearTimeout);
  expect(globalThis.clearInterval).toBe(nodeTimers.clearInterval);
});

it("defines the hook without the host's timer natives", async () => {
  delete (globalThis as { __inca_timer__?: unknown }).__inca_timer__;

  await load({});

  expect(g.__inca_timer__).toBeTypeOf("function");
});
