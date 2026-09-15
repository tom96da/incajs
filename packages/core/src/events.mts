// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { native } from "./native.mts";
import type { CallbackId, EventListener, NodeId } from "./types.mts";

/**
 * The JS half of every registration: the `callbackId` the host holds, the
 * listener at `globalThis.__gpjsui_callbacks__[callbackId]`, and which
 * `(nodeId, event)` that id belongs to.
 *
 * A registration is reached from either end — by `(nodeId, event)` when
 * replacing a listener, by `callbackId` when the host reports one released —
 * so all three move together in here, and none can be updated without the
 * others.
 */
interface Registrations {
  /**
   * Records `listener` under a fresh id.
   * @param nodeId - the node being listened on
   * @param event - the event name
   * @param listener - the function to run
   * @returns the id the host should be given
   */
  add(nodeId: NodeId, event: string, listener: EventListener): CallbackId;

  /**
   * @param nodeId - the node to look under
   * @param event - the event name
   * @returns the id registered for that pair, or `undefined`
   */
  idFor(nodeId: NodeId, event: string): CallbackId | undefined;

  /**
   * Forgets `callbackId` and the listener it named. A no-op for an id that
   * was never registered, or was released already.
   * @param callbackId - the id to drop
   */
  release(callbackId: CallbackId): void;
}

/**
 * @returns a new, empty {@link Registrations}
 */
function createRegistrations(): Registrations {
  const idByKey = new Map<string, CallbackId>();
  const keyById = new Map<CallbackId, string>();
  let nextCallbackId = 0;

  const keyOf = (nodeId: NodeId, event: string): string => `${nodeId}:${event}`;

  // Lazily, because the host defines nothing until the first registration.
  const listeners = (): Record<CallbackId, EventListener> =>
    (globalThis.__gpjsui_callbacks__ ??= {});

  return {
    add(nodeId, event, listener) {
      const callbackId = nextCallbackId++;
      listeners()[callbackId] = listener;
      idByKey.set(keyOf(nodeId, event), callbackId);
      keyById.set(callbackId, keyOf(nodeId, event));
      return callbackId;
    },

    idFor(nodeId, event) {
      return idByKey.get(keyOf(nodeId, event));
    },

    release(callbackId) {
      delete listeners()[callbackId];
      const key = keyById.get(callbackId);
      if (key === undefined) return;
      keyById.delete(callbackId);
      idByKey.delete(key);
    },
  };
}

const registrations = createRegistrations();

/**
 * Drops the JS half of every registration in `callbackIds`. The native half
 * is already gone by the time this runs — {@link destroyNode} takes both.
 * @param callbackIds - ids the host reported as released
 * @internal
 */
export function releaseCallbacks(callbackIds: readonly CallbackId[]): void {
  for (const callbackId of callbackIds) registrations.release(callbackId);
}

/**
 * Makes `listener` the one thing that runs when `event` fires on `nodeId`,
 * replacing whatever was registered for that pair before.
 *
 * The host holds a list per `(nodeId, event)` and dispatches to all of it;
 * this wrapper keeps one, the way a framework adapter composes its own
 * handlers into a single callback. Only `"click"` is wired to a real native
 * input event today; other event names are accepted but never fire.
 * @param nodeId - the node to listen on
 * @param event - the event name (e.g. `"click"`)
 * @param listener - called when the event fires
 */
export function setEventListener(nodeId: NodeId, event: string, listener: EventListener): void {
  removeEventListener(nodeId, event);
  native().addEventListener(nodeId, event, registrations.add(nodeId, event, listener));
}

/**
 * Drops whatever {@link setEventListener} registered for `(nodeId, event)`,
 * on both the native and the JS side. A no-op if nothing is registered.
 * @param nodeId - the node to stop listening on
 * @param event - the event name
 */
export function removeEventListener(nodeId: NodeId, event: string): void {
  const callbackId = registrations.idFor(nodeId, event);
  if (callbackId === undefined) return;

  native().removeEventListener(nodeId, event, callbackId);
  registrations.release(callbackId);
}
