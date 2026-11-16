// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { native } from "./native.mts";
import type { CallbackId, EventLike, NodeId } from "../types.mts";
import type { EventTargetId, HostCallback, WindowTargetId } from "./native.mts";

/**
 * Registers `callback` for `event` on a target.
 * @param target - the event target (a node id or the window target)
 * @param event - the event name
 * @param callback - what the host calls with the event object
 * @param capture - whether the listener runs while the event travels down to the target
 * @param passive - whether the host ignores `preventDefault()` calls made in the callback
 * @returns the callback id that names the registration
 * @throws TypeError when `target` is malformed or is an unknown target tag
 * @throws TypeError when a node target names no node
 * @throws RangeError when the host has issued its last callback id
 */
export function addListener(
  target: EventTargetId,
  event: string,
  callback: HostCallback,
  capture?: boolean,
  passive?: boolean,
): CallbackId {
  return native().addEventListener(target, event, callback, capture, passive);
}

/**
 * Drops one registration of {@link addListener}.
 * @param target - the event target the registration was made on
 * @param event - the event name
 * @param callbackId - the callback id {@link addListener} returned
 * @returns whether the host found the registration
 */
export function removeListener(
  target: EventTargetId,
  event: string,
  callbackId: CallbackId,
): boolean {
  return native().removeEventListener(target, event, callbackId);
}

/**
 * Dispatches `event` at a target along its path. Dispatching at an element
 * reaches its ancestors, `document` and `window`. Dispatching at `document`
 * reaches `window`, and dispatching at `window` reaches only `window`.
 * @param target - the target to dispatch at (a node id or the window target)
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

let windowTarget: WindowTargetId | undefined;

/**
 * The target that names the window in {@link addListener}, {@link removeListener} and
 * {@link dispatchEvent}. The host is asked once.
 * @returns the window target
 */
export function windowTargetId(): WindowTargetId {
  return (windowTarget ??= native().windowTargetId());
}

/**
 * Fires an untrusted `click` at `nodeId`. A disabled `button` or `input` and a node whose
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

/**
 * Returns the id of the focused node.
 * @returns the focused node's id, or `null` when no node has focus
 */
export function focusedNode(): NodeId | null {
  return native().focusedNode();
}
