// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { isRemoved } from "./lifecycle.mts";
import type { IncaEventMap, IncaEventTarget } from "../types.mts";
import type { NodeState } from "./lifecycle.mts";
import type { IncaNode } from "./types.mts";

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

/** The handler value and the listener of one event on one handle. */
interface HandlerSlot {
  held: unknown;
  listener: (this: unknown, e: unknown) => void;
}

/**
 * Builds the event handler properties as accessors, for one prototype or for `window`. A value
 * assigned while a property holds `null` registers a bubble listener through `addEventListener`
 * of the receiver, and `null` removes it. Each call builds its own getters and setters.
 * @param receive - checks the receiver of an accessor and gives its record
 * @returns the accessors, enumerable and configurable
 */
export function handlerMembers(receive: (self: unknown) => NodeState): object {
  const members = {};
  for (const event of HANDLER_EVENTS) {
    const key = `on${event}`;
    const slotOf = (state: NodeState): HandlerSlot => {
      const slots = (state.handlers ??= new Map<string, unknown>());
      let slot = slots.get(key) as HandlerSlot | undefined;
      if (slot === undefined) {
        const created: HandlerSlot = {
          held: null,
          listener(this: unknown, e: unknown): void {
            if (typeof created.held !== "function") return;
            if (Reflect.apply(created.held, this, [e]) === false) {
              (e as { preventDefault: () => void }).preventDefault();
            }
          },
        };
        slots.set(key, created);
        slot = created;
      }
      return slot;
    };
    Object.defineProperty(members, key, {
      get() {
        return slotOf(receive(this)).held;
      },
      set(value: unknown) {
        const state = receive(this);
        const slot = slotOf(state);
        if (state.kind === "element" && isRemoved(this as IncaNode)) return;
        slot.held =
          typeof value === "function" || (typeof value === "object" && value !== null)
            ? value
            : null;
        const target = this as IncaEventTarget;
        if (slot.held === null) target.removeEventListener(event, slot.listener);
        else target.addEventListener(event, slot.listener);
      },
      enumerable: true,
      configurable: true,
    });
  }
  return members;
}
