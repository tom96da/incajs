// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  isDestroyed,
  isRemoved,
  EVENT_TARGET_RULE,
  NO_EVENTS,
  prototypes,
  receiverOf,
  stateOf,
  withTargetState,
} from "./element/lifecycle.mts";
import { addEventListener, removeEventListener } from "./events.mts";
import { dispatchEvent } from "./ffi/index.mts";
import { signalTargetOf } from "./signalTarget.mts";
import type { Node } from "./element/nodeTypes.mts";
import type { EventTargetId } from "./ffi/index.mts";
import type {
  EventListenerOrEventListenerObject,
  EventTarget,
  AddEventListenerOptions,
} from "./types.mts";

/**
 * Gives the target id of a receiver: the node id of a node, and the target of an event target
 * with no node.
 * @param value - the receiver of a listener method
 * @returns the target id
 * @throws {TypeError} when `value` is no node and no event target
 */
export function eventTargetOf(value: unknown): EventTargetId {
  if (stateOf(value) === undefined) {
    const signal = signalTargetOf(value);
    if (signal !== undefined) return signal;
  }
  const state = receiverOf(value, EVENT_TARGET_RULE);
  if (state.kind !== "target") return (value as Node).__node_id;
  return typeof state.target === "function" ? state.target() : state.target!;
}

/**
 * Whether the listener methods ignore `value`. A node that left the tree for good ignores
 * `addEventListener` and `removeEventListener`.
 * @param value - the receiver
 * @returns `true` for a removed element, text node, comment or fragment
 */
function ignoresListeners(value: unknown): boolean {
  const state = stateOf(value);
  return (
    state !== undefined &&
    state.kind !== "target" &&
    state.kind !== "document" &&
    isRemoved(value as Node)
  );
}

/**
 * The listener methods of an event target. They install on `EventTarget.prototype`.
 * @returns the members
 */
export function eventTargetMembers(): EventTarget {
  return {
    addEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject<never, never> | null,
      ...rest: [options?: boolean | AddEventListenerOptions]
    ) {
      const target = eventTargetOf(this);
      if (ignoresListeners(this)) return;
      const options = rest[0];
      const given: AddEventListenerOptions =
        typeof options === "boolean" ? { capture: options } : (options ?? {});
      // Each member is read once, in the order of the dictionary, and an inherited member counts.
      const capture = given.capture;
      const once = given.once;
      const passive = given.passive;
      const signal = given.signal;
      addEventListener(target, type, listener, {
        capture,
        once,
        passive: passive ?? stateOf(this)?.passive.includes(type) ?? false,
        signal,
      });
    },

    removeEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject<never, never> | null,
      ...rest: [options?: boolean | Pick<AddEventListenerOptions, "capture">]
    ) {
      const target = eventTargetOf(this);
      if (ignoresListeners(this)) return;
      const options = rest[0];
      const capture = typeof options === "boolean" ? options : options?.capture;
      removeEventListener(target, type, listener, { capture });
    },

    dispatchEvent(event) {
      const target = eventTargetOf(this);
      const state = stateOf(this);
      if (
        state !== undefined &&
        state.kind !== "target" &&
        state.kind !== "document" &&
        isDestroyed(this as Node)
      ) {
        return true;
      }
      return dispatchEvent(target, event);
    },
  } as EventTarget;
}

/**
 * Creates an event target that is no node: an object of `EventTarget.prototype`.
 * @param target - the target id, or a function that resolves it each time a method runs
 * @param passiveByDefault - event names whose listeners are passive unless
 * their options say `passive: false`
 * @returns the target
 */
export function createEventTarget(
  target: EventTargetId | (() => EventTargetId),
  passiveByDefault: readonly string[] = NO_EVENTS,
): EventTarget {
  return withTargetState(
    Object.create(prototypes().EventTarget) as EventTarget,
    target,
    passiveByDefault,
  );
}
