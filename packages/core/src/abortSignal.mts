// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { isHandlerValue } from "./element/eventHandlers.mts";
import { addEventListener, removeEventListener } from "./events.mts";
import { signalId } from "./ffi/index.mts";
import type { SignalTarget } from "./ffi/index.mts";

/** Marks the getter of `onabort` that core defined, so a second copy of core leaves it alone. */
const OWNER = Symbol.for("incajs.onabort");

/** What an `onabort` assignment left on one signal. */
interface Slot {
  held: unknown;
  listener?: (this: unknown, event: unknown) => void;
}

/**
 * Defines the accessor `onabort` on `proto`. The first assignment of a handler registers one
 * bubble listener for `abort`, and an assignment of a value that is no handler removes it.
 * @param proto - the prototype of `AbortSignal`. The handler registers through the listener code of
 * core, so a script that replaces `addEventListener` leaves it alone.
 */
export function defineOnabort(proto: object): void {
  const existing = Object.getOwnPropertyDescriptor(proto, "onabort");
  const getter = (existing as { get?: object } | undefined)?.get;
  if (getter !== undefined && OWNER in getter) return;
  const slots = new WeakMap<object, Slot>();
  const slotOf = (self: unknown): Slot => {
    if (signalId(self) === null) {
      throw new TypeError("the receiver is not an AbortSignal. call the method on an instance");
    }
    let slot = slots.get(self as object);
    if (slot === undefined) slots.set(self as object, (slot = { held: null }));
    return slot;
  };
  const accessor = Object.getOwnPropertyDescriptor(
    {
      get onabort() {
        return slotOf(this).held;
      },
      set onabort(value: unknown) {
        const slot = slotOf(this);
        slot.held = isHandlerValue(value) ? value : null;
        if (slot.held === null) {
          if (slot.listener !== undefined) {
            removeEventListener(this as SignalTarget, "abort", slot.listener as never);
          }
          slot.listener = undefined;
        } else if (slot.listener === undefined) {
          slot.listener = function (this: unknown, event: unknown): void {
            if (typeof slot.held === "function") Reflect.apply(slot.held, this, [event]);
          };
          addEventListener(this as SignalTarget, "abort", slot.listener as never);
        }
      },
    },
    "onabort",
  )!;
  Object.defineProperty((accessor as { get: object }).get, OWNER, { value: true });
  Object.defineProperty(proto, "onabort", accessor);
}

if (globalThis.__inca_native__ !== undefined) {
  defineOnabort(AbortSignal.prototype);
}
