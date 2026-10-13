// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { HostCallback } from "./native.mts";
import type { CallbackId, EventListenerOrObject, NodeId } from "./types.mts";

/**
 * Finds the id of a registration from its group and listener, when a listener
 * is added or removed.
 */
const idsByGroup = new Map<string, Map<EventListenerOrObject, CallbackId>>();
/**
 * Finds the group and listener of a registration from its id, when the host
 * reports the id released.
 */
const ownerById = new Map<CallbackId, { group: string; listener: EventListenerOrObject }>();

// Lazily, because the host defines nothing until the first registration.
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
  listener: EventListenerOrObject,
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
  listener: EventListenerOrObject,
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
 * The capture flag is part of the identity: the same node, event and function
 * can listen in both phases side by side.
 * @param nodeId - the node listened on
 * @param event - the event name
 * @param capture - the capture flag of the registration
 * @returns the node, event and capture flag as one string
 */
export const groupOf = (nodeId: NodeId, event: string, capture: boolean | undefined): string =>
  `${nodeId}:${event}:${capture === true}`;

/**
 * @returns how many `(nodeId, event, capture)` groups hold a registration
 * @internal
 */
export function registeredGroupCount(): number {
  return idsByGroup.size;
}
