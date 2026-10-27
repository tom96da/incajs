// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { native } from "./native.mts";

declare global {
  function setTimeout<A extends unknown[]>(
    handler: (...args: A) => void,
    delay?: number,
    ...args: A
  ): number;
  function setInterval<A extends unknown[]>(
    handler: (...args: A) => void,
    delay?: number,
    ...args: A
  ): number;
  function clearTimeout(id?: number): void;
  function clearInterval(id?: number): void;
}

/** What a queued timer runs. */
interface Entry {
  handler: (...args: unknown[]) => unknown;
  args: unknown[];
  repeat: boolean;
}

/** The queued timers by the id the host returned. A one-shot leaves when it runs. */
const entries = new Map<number, Entry>();

/**
 * Queues `handler` with the host.
 * @param name - the function the caller called, for the error message
 * @param handler - what the timer runs
 * @param delay - milliseconds, read as a 32-bit integer and raised to 0
 * @param args - the arguments `handler` receives
 * @param repeat - whether the timer runs again every `delay` milliseconds
 * @returns the id the host returned
 */
function start(
  name: string,
  handler: unknown,
  delay: unknown,
  args: unknown[],
  repeat: boolean,
): number {
  if (typeof handler !== "function")
    throw new TypeError(`${name} takes a function. pass a function as the handler`);
  const ms = (delay as number) | 0;
  const id = native().startTimer(ms > 0 ? ms : 0, repeat);
  if (id === undefined) throw new RangeError("timer ids are exhausted. restart the app");
  entries.set(id, { handler: handler as Entry["handler"], args, repeat });
  return id;
}

/**
 * Drops the timer `id` names. An id no timer holds does nothing.
 * @param id - the id, read as a 32-bit integer
 */
function clear(id: unknown): void {
  const key = (id as number) | 0;
  if (entries.delete(key)) native().stopTimer(key);
}

/**
 * The host calls this for each due timer. A one-shot leaves the table before
 * its handler runs. A throw of the handler reaches the host, which reports it.
 * An id this load of core does not hold, such as a timer of an earlier load,
 * is stopped.
 */
const due: typeof globalThis.__inca_timer__ = (id) => {
  const entry = entries.get(id);
  if (entry === undefined) {
    native().stopTimer(id);
    return;
  }
  if (!entry.repeat) entries.delete(id);
  Reflect.apply(entry.handler, globalThis, entry.args);
};

/** The functions this module defines on `globalThis`. */
const timers = {
  setTimeout(handler: unknown, delay: unknown = 0, ...args: unknown[]): number {
    return start("setTimeout", handler, delay, args, false);
  },
  setInterval(handler: unknown, delay: unknown = 0, ...args: unknown[]): number {
    return start("setInterval", handler, delay, args, true);
  },
  clearTimeout(id: unknown = 0): void {
    clear(id);
  },
  clearInterval(id: unknown = 0): void {
    clear(id);
  },
};

Object.defineProperty(globalThis, "__inca_timer__", { value: due, configurable: true });
// Outside the host, Node's and the test runner's timers stay in place.
if (typeof globalThis.__inca_native__?.startTimer === "function") Object.assign(globalThis, timers);
