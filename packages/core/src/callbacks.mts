// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { markFreed } from "./element/lifecycle.mts";
import type { EventTargetId, HostCallback } from "./native.mts";
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

/** The table the host creates at install; a realm without the host gets a plain one. */
const callables = (): Record<CallbackId, HostCallback> => (globalThis.__inca_callbacks__ ??= {});

/**
 * Records `callable` under `callbackId` at `globalThis.__inca_callbacks__`.
 * @param group - the node, event and capture flag as one string, from {@link groupOf}
 * @param listener - the listener the caller registered, which identifies the registration
 * @param callbackId - the id the host returned for the registration
 * @param callable - the function the host runs
 */
export function addCallback(
  group: string,
  listener: EventListenerOrObject<never, never>,
  callbackId: CallbackId,
  callable: HostCallback,
): void {
  callables()[callbackId] = callable;
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
  delete callables()[callbackId];
  const owner = ownerById.get(callbackId);
  if (owner === undefined) return;
  ownerById.delete(callbackId);
  const ids = idsByGroup.get(owner.group);
  ids?.delete(owner.listener);
  if (ids?.size === 0) idsByGroup.delete(owner.group);
}

/**
 * Drops the JS half of every registration in `callbackIds`. The native half
 * is already gone by the time this runs: `destroyNode` takes both.
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
 * no parent. Each callback is released on its own, so one that throws does not
 * skip the rest. The first error is thrown after the rest is done. Core
 * defines it read-only on each load, so the latest copy of core answers.
 */
const released: typeof globalThis.__inca_released__ = (nodeIds, callbackIds) => {
  let failure: unknown;
  let failed = false;
  for (const callbackId of callbackIds) {
    try {
      releaseCallbacks([callbackId]);
    } catch (error) {
      if (!failed) failure = error;
      failed = true;
    }
  }
  markFreed(nodeIds);
  if (failed) throw failure;
};

Object.defineProperty(globalThis, "__inca_released__", { value: released, configurable: true });
