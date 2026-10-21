// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { addEventListener, dispatchEvent, removeEventListener } from "./events.mts";
import type { EventListenerOrObject, IncaEventTarget, ListenerOptions, NodeId } from "./types.mts";

/**
 * Builds the listener methods of an event target that stands for one node.
 * @param nodeId - resolves the node when a method runs
 * @param passiveByDefault - event names whose listeners are passive unless
 * their options say `passive: false`
 * @returns the target's `addEventListener`, `removeEventListener` and
 * `dispatchEvent`
 */
export function createEventTarget(
  nodeId: () => NodeId,
  passiveByDefault: readonly string[] = [],
): IncaEventTarget {
  return {
    addEventListener(
      type: string,
      listener: EventListenerOrObject<never, never> | null,
      options?: boolean | ListenerOptions,
    ) {
      const given: ListenerOptions =
        typeof options === "boolean" ? { capture: options } : (options ?? {});
      addEventListener(nodeId(), type, listener, {
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
      removeEventListener(nodeId(), type, listener, { capture });
    },

    dispatchEvent: (event) => dispatchEvent(nodeId(), event),
  };
}
