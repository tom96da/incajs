// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { addEventListener, dispatchEvent, removeEventListener } from "./events.mts";
import type { EventTargetId } from "./native.mts";
import type { EventListenerOrObject, IncaEventTarget, ListenerOptions } from "./types.mts";

/**
 * Builds the listener methods of an event target: a node, document or window.
 * @param target - the target id, or a function that resolves it each time a method runs
 * @param passiveByDefault - event names whose listeners are passive unless
 * their options say `passive: false`
 * @returns the target's `addEventListener`, `removeEventListener` and
 * `dispatchEvent`
 */
export function createEventTarget(
  target: EventTargetId | (() => EventTargetId),
  passiveByDefault: readonly string[] = [],
): IncaEventTarget {
  const getTarget = typeof target === "function" ? target : () => target;
  return {
    addEventListener(
      type: string,
      listener: EventListenerOrObject<never, never> | null,
      options?: boolean | ListenerOptions,
    ) {
      const given: ListenerOptions =
        typeof options === "boolean" ? { capture: options } : (options ?? {});
      addEventListener(getTarget(), type, listener, {
        ...given,
        passive: given.passive ?? passiveByDefault.includes(type),
      });
    },

    removeEventListener(
      type: string,
      listener: EventListenerOrObject<never, never> | null,
      options?: boolean | Pick<ListenerOptions, "capture">,
    ) {
      const capture = typeof options === "boolean" ? options : options?.capture;
      removeEventListener(getTarget(), type, listener, { capture });
    },

    dispatchEvent: (event) => dispatchEvent(getTarget(), event),
  };
}
