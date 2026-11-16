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
import type { IncaNode } from "./element/types.mts";
import type { EventTargetId } from "./ffi/index.mts";
import type { EventListenerOrObject, IncaEventTarget, ListenerOptions } from "./types.mts";

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
  if (state.kind !== "target") return (value as IncaNode).__node_id;
  return typeof state.target === "function" ? state.target() : state.target!;
}

/**
 * Whether the listener methods ignore `value`: a node that left the tree for good ignores
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
    isRemoved(value as IncaNode)
  );
}

/**
 * The listener methods of an event target. They install on `EventTarget.prototype`.
 * @returns the members
 */
export function eventTargetMembers(): IncaEventTarget {
  return {
    addEventListener(
      type: string,
      listener: EventListenerOrObject<never, never> | null,
      ...rest: [options?: boolean | ListenerOptions]
    ) {
      const target = eventTargetOf(this);
      if (ignoresListeners(this)) return;
      const options = rest[0];
      const given: ListenerOptions =
        typeof options === "boolean" ? { capture: options } : (options ?? {});
      addEventListener(target, type, listener, {
        ...given,
        passive: given.passive ?? stateOf(this)?.passive.includes(type) ?? false,
      });
    },

    removeEventListener(
      type: string,
      listener: EventListenerOrObject<never, never> | null,
      ...rest: [options?: boolean | Pick<ListenerOptions, "capture">]
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
        isDestroyed(this as IncaNode)
      ) {
        return true;
      }
      return dispatchEvent(target, event);
    },
  } as IncaEventTarget;
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
): IncaEventTarget {
  return withTargetState(
    Object.create(prototypes().EventTarget) as IncaEventTarget,
    target,
    passiveByDefault,
  );
}
