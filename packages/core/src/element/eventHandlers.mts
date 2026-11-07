// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { IncaEventMap, IncaEventTarget } from "../types.mts";
import type { EventHandlers } from "./types.mts";

/** Every event name but the ones that have no handler property. */
type HandlerEvent = Exclude<
  keyof IncaEventMap,
  "focusin" | "focusout" | "compositionstart" | "compositionupdate" | "compositionend"
>;

/** The events whose handler property every element has. */
export const HANDLER_EVENTS = (<const U extends readonly HandlerEvent[]>(
  // A name left out of the list makes the call fail with a wrong argument count.
  ...events: [Exclude<HandlerEvent, U[number]>] extends [never] ? U : [missing: HandlerEvent]
) => events as unknown as U)(
  "auxclick",
  "beforeinput",
  "blur",
  "change",
  "click",
  "contextmenu",
  "dblclick",
  "focus",
  "gotpointercapture",
  "input",
  "keydown",
  "keypress",
  "keyup",
  "lostpointercapture",
  "mousedown",
  "mouseenter",
  "mouseleave",
  "mousemove",
  "mouseout",
  "mouseover",
  "mouseup",
  "pointerdown",
  "pointerenter",
  "pointerleave",
  "pointermove",
  "pointerout",
  "pointerover",
  "pointerup",
  "scroll",
  "wheel",
);

/**
 * Defines the event handler properties on `handle`. A value assigned while a property holds `null`
 * registers a bubble listener through `handle.addEventListener`, and `null` removes it.
 * @param handle - the element handle
 * @param ignored - whether an assignment is ignored, as it is once the node is removed
 * @returns `handle`, with the properties
 */
export function withEventHandlers<T extends IncaEventTarget>(
  handle: T,
  ignored: () => boolean,
): T & EventHandlers {
  for (const event of HANDLER_EVENTS) {
    let held: unknown = null;
    const listener = function (this: unknown, e: unknown): void {
      if (typeof held !== "function") return;
      if (Reflect.apply(held, this, [e]) === false) {
        (e as { preventDefault: () => void }).preventDefault();
      }
    };
    Object.defineProperty(handle, `on${event}`, {
      get: () => held,
      set(value: unknown) {
        if (ignored()) return;
        held =
          typeof value === "function" || (typeof value === "object" && value !== null)
            ? value
            : null;
        if (held === null) handle.removeEventListener(event, listener);
        else handle.addEventListener(event, listener);
      },
    });
  }
  return handle as T & EventHandlers;
}
