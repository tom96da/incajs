// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { addCallback, callbackIdFor, groupOf, releaseCallback } from "./callbacks.mts";
import { native, type HostCallback } from "./native.mts";
import type { EventLike, EventListenerOrObject, ListenerOptions, NodeId } from "./types.mts";

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
  if (callbackIdFor(group, listener) !== undefined) return;

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
  const callbackId = addCallback(group, listener, tracking(run));
  try {
    native().addEventListener(
      nodeId,
      event,
      callbackId,
      options.capture === true,
      options.passive === true,
    );
  } catch (error) {
    releaseCallback(callbackId);
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
  const callbackId = callbackIdFor(groupOf(nodeId, event, options.capture), listener);
  if (callbackId === undefined) return;

  native().removeEventListener(nodeId, event, callbackId);
  releaseCallback(callbackId);
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
 * is optional. The scroll containers around the node scroll it into view.
 * @param nodeId - the node to focus
 * @param preventScroll - `true` to leave the scroll containers in place; omitted or `false` scrolls the node into view
 */
export function focus(nodeId: NodeId, preventScroll?: boolean): void {
  if (preventScroll === undefined) native().focusNode(nodeId);
  else native().focusNode(nodeId, preventScroll);
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
