// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { native } from "./native.mts";
import type { CallbackId, EventLike, NodeId } from "../types.mts";
import type { EventTargetId, HostCallback, SignalTarget, WindowTargetId } from "./native.mts";

/**
 * Registers `callback` for `event` on a target.
 * @param target - the event target (a node id, the window target or a signal object)
 * @param event - the event name
 * @param callback - what the host calls with the event object
 * @param capture - whether the listener runs while the event travels down to the target
 * @param passive - whether the host ignores `preventDefault()` calls made in the callback
 * @param signal - a signal that removes the registration when it aborts
 * @returns the callback id that names the registration, or `null` when `signal` is aborted
 * already, which registers nothing
 * @throws TypeError when `target` is malformed or is an unknown target tag
 * @throws TypeError when a node target names no node
 * @throws TypeError when `signal` is no signal of the host
 * @throws RangeError when the host has issued its last callback id
 */
export function addListener(
  target: EventTargetId,
  event: string,
  callback: HostCallback,
  capture?: boolean,
  passive?: boolean,
  signal?: SignalTarget,
): CallbackId | null {
  return native().addEventListener(target, event, callback, capture, passive, signal);
}

/**
 * Drops one registration of {@link addListener}.
 * @param target - the event target the registration was made on (a node id, the window target or
 * a signal object)
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
 * reaches `window`, and dispatching at `window` reaches only `window`. Dispatching at a signal
 * reaches only that signal.
 * @param target - the target to dispatch at (a node id, the window target or a signal object)
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
 * The id the host gave an `AbortSignal` object.
 * @param value - any value
 * @returns the id, or `null` for a value that is no signal of the host
 */
export function signalId(value: unknown): number | null {
  return native().signalId(value);
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
 * Returns the id of the focused node.
 * @returns the focused node's id, or `null` when no node has focus
 */
export function focusedNode(): NodeId | null {
  return native().focusedNode();
}
