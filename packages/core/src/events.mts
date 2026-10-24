// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { addCallback, callbackIdFor, groupOf, releaseCallback } from "./callbacks.mts";
import { native, type EventTargetId, type HostCallback } from "./native.mts";
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
 * Registers `listener` for `event` on the target (node, document or window).
 * Registering the same listener for the same target, event and `capture`
 * again changes nothing, and the first registration keeps its other options.
 * A `null` listener is ignored.
 *
 * Any event name is accepted, and it fires when the host dispatches that name.
 * `IncaEventMap` gives the event type of each name the host dispatches.
 * @param target - the event target (node id, "$document" or "$window")
 * @param event - the event name (e.g. `"click"`)
 * @param listener - a function, called with the event's `currentTarget` as
 * `this`, or an object whose `handleEvent` method is called with the object
 * as `this` when the event fires
 * @param options - `capture` listens while the event travels down to the
 * target, `passive` ignores `preventDefault()` calls in `listener`, and
 * `once` removes the listener before it first runs
 * @throws TypeError when `target` is malformed or is an unknown target tag
 * @throws TypeError when a node target names no node
 * @throws RangeError when the host has issued its last callback id
 */
export function addEventListener(
  target: EventTargetId,
  event: string,
  listener: EventListenerOrObject<never, never> | null,
  options: ListenerOptions = {},
): void {
  if (listener === null || listener === undefined) return;
  const group = groupOf(target, event, options.capture);
  if (callbackIdFor(group, listener) !== undefined) return;

  const call: HostCallback = (...args) =>
    typeof listener === "function"
      ? (listener as HostCallback).apply(
          (args[0] as { currentTarget?: unknown } | null | undefined)?.currentTarget,
          args,
        )
      : (listener.handleEvent as HostCallback).apply(listener, args);
  const run: HostCallback = options.once
    ? (...args) => {
        removeEventListener(target, event, listener, options);
        return call(...args);
      }
    : call;
  const callbackId = native().addEventListener(
    target,
    event,
    options.capture === true,
    options.passive === true,
  );
  addCallback(group, listener, callbackId, tracking(run));
}

/**
 * Drops the registration {@link addEventListener} made for `listener` on
 * the target with the same `event` and `capture`. A no-op if there is none.
 * @param target - the event target (node, document or window)
 * @param event - the event name
 * @param listener - the listener that was registered
 * @param options - `capture` of the registration to drop
 */
export function removeEventListener(
  target: EventTargetId,
  event: string,
  listener: EventListenerOrObject<never, never> | null,
  options: Pick<ListenerOptions, "capture"> = {},
): void {
  if (listener === null || listener === undefined) return;
  const callbackId = callbackIdFor(groupOf(target, event, options.capture), listener);
  if (callbackId === undefined) return;

  native().removeEventListener(target, event, callbackId);
  releaseCallback(callbackId);
}

/**
 * Dispatches `event` at a target along its path. Dispatching at an element
 * reaches its ancestors, `document` and `window`. Dispatching at `document`
 * reaches `window`, and dispatching at `window` reaches only `window`.
 * @param target - the target to dispatch at (node id, "$document" or "$window")
 * @param event - the event to dispatch
 * @returns `false` when a listener canceled a cancelable event, else `true`
 * @throws TypeError when `target` is malformed or is an unknown target tag
 * @throws TypeError when `target` is a node id that names no node
 * @throws TypeError when `event` is not an event built with an event constructor
 * @throws DOMException named `InvalidStateError` when `event` is being dispatched
 */
export function dispatchEvent(target: EventTargetId, event: EventLike): boolean {
  return native().dispatchEvent(target, event);
}

/**
 * Fires an untrusted `click` at `nodeId`. A disabled `button` and a node whose
 * own click is still running ignore the call.
 * @param nodeId - the node to click
 * @throws TypeError when `nodeId` is malformed
 * @throws TypeError when `nodeId` names no node
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
 * @throws TypeError when `nodeId` is malformed
 * @throws TypeError when `nodeId` names no node
 */
export function focus(nodeId: NodeId, preventScroll?: boolean): void {
  native().focusNode(nodeId, preventScroll);
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
 * Captures the mouse pointer for `nodeId` while a button is held. The pointer events and the
 * mouse events that follow go to the node until the last button is released, and
 * `gotpointercapture` fires before the next pointer event. With no button held the call does
 * nothing.
 * @param nodeId - the node to capture the pointer for
 * @param pointerId - the pointer to capture; only `1`, the mouse, exists
 * @throws DOMException named `NotFoundError` when `pointerId` is not `1`
 * @throws DOMException named `InvalidStateError` when `nodeId` names no node or a node outside
 * the mounted tree
 */
export function setPointerCapture(nodeId: NodeId, pointerId: number): void {
  native().setPointerCapture(nodeId, pointerId);
}

/**
 * Releases the pointer capture `nodeId` requested. `lostpointercapture` fires before the next
 * pointer event. The call does nothing for a node that has no capture.
 * @param nodeId - the node that holds the capture
 * @param pointerId - the pointer to release; only `1`, the mouse, exists
 * @throws DOMException named `NotFoundError` when `pointerId` is not `1`
 */
export function releasePointerCapture(nodeId: NodeId, pointerId: number): void {
  native().releasePointerCapture(nodeId, pointerId);
}

/**
 * Whether `nodeId` requested the pointer capture and has not released it.
 * @param nodeId - the node to ask about
 * @param pointerId - the pointer to ask about
 * @returns `false` for a pointer other than `1`, for a node outside the mounted tree and for a
 * node that holds no capture
 */
export function hasPointerCapture(nodeId: NodeId, pointerId: number): boolean {
  return native().hasPointerCapture(nodeId, pointerId);
}
