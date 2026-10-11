// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { native, type HostCallback } from "./native.mts";
import type {
  CallbackId,
  EventLike,
  EventListenerOrObject,
  ListenerOptions,
  NodeId,
} from "./types.mts";

/**
 * The JS half of every registration: the `callbackId` the host holds, the
 * callback at `globalThis.__inca_callbacks__[callbackId]`, and the
 * `(nodeId, event, capture)` group and listener that id belongs to.
 *
 * A registration is reached from either end: by group and listener when a
 * listener is added or removed, and by `callbackId` when the host reports
 * one released. All of it moves together in here.
 */
interface Registrations {
  /**
   * Records `callable` under a fresh id.
   * @param group - the node, event and capture flag as one string
   * @param listener - the listener the caller registered, which identifies
   * the registration
   * @param callable - the function the host runs
   * @returns the id the host should be given
   */
  add(group: string, listener: EventListenerOrObject, callable: HostCallback): CallbackId;

  /**
   * @param group - the node, event and capture flag as one string
   * @param listener - the listener the caller registered
   * @returns the id registered for the pair, or `undefined`
   */
  idFor(group: string, listener: EventListenerOrObject): CallbackId | undefined;

  /**
   * Forgets `callbackId` and the listener it named. A no-op for an id that
   * was never registered, or was released already.
   * @param callbackId - the id to drop
   */
  release(callbackId: CallbackId): void;

  /** @returns how many `(nodeId, event, capture)` groups hold a registration */
  groupCount(): number;
}

/**
 * @returns a new, empty {@link Registrations}
 */
function createRegistrations(): Registrations {
  const idsByGroup = new Map<string, Map<EventListenerOrObject, CallbackId>>();
  const ownerById = new Map<CallbackId, { group: string; listener: EventListenerOrObject }>();
  let nextCallbackId = 0;

  // Lazily, because the host defines nothing until the first registration.
  const callables = (): Record<CallbackId, HostCallback> => (globalThis.__inca_callbacks__ ??= {});

  return {
    add(group, listener, callable) {
      const callbackId = nextCallbackId++;
      callables()[callbackId] = callable;
      let ids = idsByGroup.get(group);
      if (!ids) idsByGroup.set(group, (ids = new Map()));
      ids.set(listener, callbackId);
      ownerById.set(callbackId, { group, listener });
      return callbackId;
    },

    idFor(group, listener) {
      return idsByGroup.get(group)?.get(listener);
    },

    release(callbackId) {
      delete callables()[callbackId];
      const owner = ownerById.get(callbackId);
      if (owner === undefined) return;
      ownerById.delete(callbackId);
      const ids = idsByGroup.get(owner.group);
      ids?.delete(owner.listener);
      if (ids?.size === 0) idsByGroup.delete(owner.group);
    },

    groupCount: () => idsByGroup.size,
  };
}

const registrations = createRegistrations();

/**
 * The capture flag is part of the identity: the same node, event and function
 * can listen in both phases side by side.
 */
const groupOf = (nodeId: NodeId, event: string, capture: boolean | undefined): string =>
  `${nodeId}:${event}:${capture === true}`;

/**
 * @returns how many `(nodeId, event, capture)` groups hold a registration
 * @internal
 */
export function registeredGroupCount(): number {
  return registrations.groupCount();
}

let latestId = 0;

/**
 * @returns the `eventId` of the newest event any registered listener has
 * received, or 0 before the first. Ids grow with each event, and every
 * listener of one event receives the same id.
 */
export function latestEventId(): number {
  return latestId;
}

/** Records the event's `eventId` before the listener runs. */
function tracking(listener: HostCallback): HostCallback {
  return (...args) => {
    const id = (args[0] as { eventId?: unknown } | null | undefined)?.eventId;
    if (typeof id === "number" && id > latestId) latestId = id;
    return listener(...args);
  };
}

/**
 * Drops the JS half of every registration in `callbackIds`. The native half
 * is already gone by the time this runs — `destroyNode` takes both.
 * @param callbackIds - ids the host reported as released
 * @internal
 */
export function releaseCallbacks(callbackIds: readonly CallbackId[]): void {
  for (const callbackId of callbackIds) registrations.release(callbackId);
}

/**
 * Registers `listener` for `event` on `nodeId`. Registering the same listener
 * for the same node, event and `capture` again changes nothing, and the first
 * registration keeps its other options. A `null` listener is ignored.
 *
 * Any event name is accepted, and it fires when the host dispatches that name.
 * See the `EventListener` type for the fields an event carries.
 * @param nodeId - the node to listen on
 * @param event - the event name (e.g. `"click"`)
 * @param listener - a function, or an object whose `handleEvent` method is
 * called with the object as `this` when the event fires
 * @param options - `capture` listens while the event travels down to the
 * target, `passive` ignores `preventDefault()` calls in `listener`, and
 * `once` removes the listener before it first runs
 * @throws TypeError when `nodeId` is malformed
 * @throws TypeError when `nodeId` names no node
 */
export function addEventListener(
  nodeId: NodeId,
  event: string,
  listener: EventListenerOrObject | null,
  options: ListenerOptions = {},
): void {
  if (listener === null || listener === undefined) return;
  const group = groupOf(nodeId, event, options.capture);
  if (registrations.idFor(group, listener) !== undefined) return;

  const call: HostCallback = (...args) =>
    typeof listener === "function"
      ? (listener as HostCallback)(...args)
      : (listener.handleEvent as HostCallback).apply(listener, args);
  const run: HostCallback = options.once
    ? (...args) => {
        removeEventListener(nodeId, event, listener, options);
        return call(...args);
      }
    : call;
  const callbackId = registrations.add(group, listener, tracking(run));
  try {
    native().addEventListener(
      nodeId,
      event,
      callbackId,
      options.capture === true,
      options.passive === true,
    );
  } catch (error) {
    registrations.release(callbackId);
    throw error;
  }
}

/**
 * Drops the registration {@link addEventListener} made for `listener` on
 * `nodeId` with the same `event` and `capture`. A no-op if there is none.
 * @param nodeId - the node to stop listening on
 * @param event - the event name
 * @param listener - the listener that was registered
 * @param options - `capture` of the registration to drop
 */
export function removeEventListener(
  nodeId: NodeId,
  event: string,
  listener: EventListenerOrObject | null,
  options: Pick<ListenerOptions, "capture"> = {},
): void {
  if (listener === null || listener === undefined) return;
  const callbackId = registrations.idFor(groupOf(nodeId, event, options.capture), listener);
  if (callbackId === undefined) return;

  native().removeEventListener(nodeId, event, callbackId);
  registrations.release(callbackId);
}

/**
 * Dispatches `event` at `nodeId` along the path to the root.
 * @param nodeId - the node to dispatch at
 * @param event - the event to dispatch
 * @returns `false` when a listener canceled a cancelable event, else `true`
 * @throws TypeError when `nodeId` is malformed
 * @throws TypeError when `nodeId` names no node
 * @throws TypeError when `event` is not an `Event` or `CustomEvent`
 * @throws DOMException named `InvalidStateError` when `event` is being dispatched
 */
export function dispatchEvent(nodeId: NodeId, event: EventLike): boolean {
  return native().dispatchEvent(nodeId, event);
}

/**
 * Fires an untrusted `click` at `nodeId`. A disabled `button`, a node whose
 * own click is still running and an unknown node ignore the call.
 * @param nodeId - the node to click
 * @throws TypeError when `nodeId` is malformed
 */
export function click(nodeId: NodeId): void {
  native().click(nodeId);
}

/**
 * Requests that `nodeId` become focused. Takes effect on the next frame,
 * dispatching `"blur"`/`"focus"` for whatever actually changed. The node
 * needs a `tabindex` attribute to take focus, and a listener for either event
 * is optional.
 * @param nodeId - the node to focus
 */
export function focus(nodeId: NodeId): void {
  native().focusNode(nodeId);
}

/**
 * Requests that `nodeId` become unfocused, the way `element.blur()` does.
 * Takes effect on the next frame; a no-op if `nodeId` isn't the focused node
 * when it runs.
 * @param nodeId - the node to unfocus
 */
export function blur(nodeId: NodeId): void {
  native().blurNode(nodeId);
}

/**
 * Returns the id of the focused node.
 * @returns the focused node's id, or `null` when no node has focus
 */
export function focusedNode(): NodeId | null {
  return native().focusedNode();
}
