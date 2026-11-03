// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { markFreed } from "./element/lifecycle.mts";
import type { EventTargetId } from "./native.mts";
import type { CallbackId, EventListenerOrObject } from "./types.mts";

/**
 * Finds the id of a registration from its group and listener, when a listener
 * is added or removed.
 */
const idsByGroup = new Map<string, Map<EventListenerOrObject<never, never>, CallbackId>>();
/**
 * Finds the group and listener of a registration from its id, when the host
 * reports the id released.
 */
const ownerById = new Map<
  CallbackId,
  { group: string; listener: EventListenerOrObject<never, never> }
>();

/**
 * Records the registration of `listener` under `callbackId`. The host keeps the function it runs.
 * @param group - the node, event and capture flag as one string, from {@link groupOf}
 * @param listener - the listener the caller registered, which identifies the registration
 * @param callbackId - the id the host returned for the registration
 */
export function addCallback(
  group: string,
  listener: EventListenerOrObject<never, never>,
  callbackId: CallbackId,
): void {
  let ids = idsByGroup.get(group);
  if (!ids) idsByGroup.set(group, (ids = new Map()));
  ids.set(listener, callbackId);
  ownerById.set(callbackId, { group, listener });
}

/**
 * @param group - the node, event and capture flag as one string
 * @param listener - the listener the caller registered
 * @returns the id registered for the pair, or `undefined`
 */
export function callbackIdFor(
  group: string,
  listener: EventListenerOrObject<never, never>,
): CallbackId | undefined {
  return idsByGroup.get(group)?.get(listener);
}

/**
 * Forgets `callbackId` and the listener it names. An id that was never
 * registered, or was released already, changes nothing.
 * @param callbackId - the id to drop
 */
export function releaseCallback(callbackId: CallbackId): void {
  const owner = ownerById.get(callbackId);
  if (owner === undefined) return;
  ownerById.delete(callbackId);
  const ids = idsByGroup.get(owner.group);
  ids?.delete(owner.listener);
  if (ids?.size === 0) idsByGroup.delete(owner.group);
}

/**
 * Drops the JS half of every registration in `callbackIds`. The native half is already gone by
 * the time this runs: `releaseListeners` and the sweep take both.
 * @param callbackIds - ids the host reported as released
 * @internal
 */
export function releaseCallbacks(callbackIds: readonly CallbackId[]): void {
  for (const callbackId of callbackIds) releaseCallback(callbackId);
}

/**
 * The capture flag is part of the identity: the same target, event and
 * function can listen in both phases side by side.
 * @param target - the event target (node id, "$document" or "$window")
 * @param event - the event name
 * @param capture - the capture flag of the registration
 * @returns the target, event and capture flag as one string
 */
export const groupOf = (
  target: EventTargetId,
  event: string,
  capture: boolean | undefined,
): string => `${target}:${event}:${capture === true}`;

/**
 * @returns how many `(target, event, capture)` groups hold a registration
 * @internal
 */
export function registeredGroupCount(): number {
  return idsByGroup.size;
}

/**
 * The host calls this at the start of a frame, after it freed nodes that had
 * no parent. Core defines it read-only on each load, so the latest copy of
 * core answers.
 */
const released: typeof globalThis.__inca_released__ = (nodeIds, callbackIds) => {
  releaseCallbacks(callbackIds);
  markFreed(nodeIds);
};

Object.defineProperty(globalThis, "__inca_released__", { value: released, configurable: true });
