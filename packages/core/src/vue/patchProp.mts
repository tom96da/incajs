// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  callWithAsyncErrorHandling,
  ErrorCodes,
  type ComponentInternalInstance,
  type RendererOptions,
} from "@vue/runtime-core";

import { isDestroyed } from "../element/lifecycle.mts";
import type { IncaElement } from "../element/types.mts";
import type { IncaCore, EventListener, ListenerOptions } from "../rendererCore.mts";

/**
 * Whether a prop is a native event. `onUpdate:modelValue` and the like are
 * component events, never native ones.
 */
const isOn = (key: string): boolean => /^on[^a-z]/.test(key) && !key.startsWith("onUpdate:");

/**
 * The suffix Vue's SFC compiler appends per event modifier: `@click.once`
 * becomes `onClickOnce`, `@click.once.capture` becomes `onClickOnceCapture`.
 * Matches `@vue/runtime-dom`'s `modules/events.ts`.
 */
const modifierSuffixRE = /(?:Once|Passive|Capture)$/;
/**
 * A key that is a bare modifier name is the event of that name.
 */
const bareModifierRE = /^on:?(?:Once|Passive|Capture)$/;

/** Same as `@vue/shared`'s `hyphenate`; core does not depend on `vue`. */
const hyphenate = (str: string): string => str.replace(/\B([A-Z])/g, "-$1").toLowerCase();

interface ParsedEventKey extends ListenerOptions {
  event: string;
  once: boolean;
}

/**
 * Strips modifier suffixes off `rawKey`, repeatedly since Vue can combine
 * several, then reads what's left as the event name: `on:name` keeps the
 * name as written, `onXxx` is hyphenated (`onMyEvent` becomes `my-event`).
 */
function parseEventKey(rawKey: string): ParsedEventKey {
  let key = rawKey;
  const parsed: ParsedEventKey = { event: "", once: false, capture: false, passive: false };
  let match: RegExpMatchArray | null;
  while ((match = key.match(modifierSuffixRE)) && !bareModifierRE.test(key)) {
    key = key.slice(0, -match[0].length);
    if (match[0] === "Once") parsed.once = true;
    else if (match[0] === "Capture") parsed.capture = true;
    else parsed.passive = true;
  }
  parsed.event = key[2] === ":" ? key.slice(3) : hyphenate(key.slice(2));
  return parsed;
}

// `setStyle` raises on a non-primitive value, the same way `setAttribute`
// does, so a value shape it can't take (e.g. an array, for DOM's multi-value
// CSS properties) is never sent.
const isSendable = (value: unknown): value is string | number =>
  typeof value === "string" || typeof value === "number";

// What native holds per element, keyed by style key. Vue can pass the same
// object as both the previous and next value after an in-place mutation, so
// the diff runs against this snapshot instead of the previous value.
const sentStyles = new WeakMap<IncaElement, Map<string, string | number>>();

function patchStyle(core: IncaCore, el: IncaElement, nextValue: unknown): void {
  const next =
    typeof nextValue === "object" && nextValue !== null
      ? (nextValue as Record<string, unknown>)
      : {};
  let sent = sentStyles.get(el);
  if (!sent) sentStyles.set(el, (sent = new Map()));

  let removed = false;
  for (const key of sent.keys()) {
    if (!isSendable(next[key])) {
      core.removeStyle(el.id, key);
      sent.delete(key);
      removed = true;
    }
  }
  // A removal can clear a property that another spelling of the key still sets.
  for (const [key, value] of Object.entries(next)) {
    if (isSendable(value) && (removed || sent.get(key) !== value)) {
      core.setStyle(el.id, key, value);
      sent.set(key, value);
    }
  }
}

// `@vue/runtime-core` types an `onXxx` prop as `Function | Function[]`, so
// several handlers can arrive for one event.
function asHandlers(value: unknown): EventListener[] | null {
  const handlers = (Array.isArray(value) ? value : [value]).filter(
    (entry): entry is EventListener => typeof entry === "function",
  );
  return handlers.length === 0 ? null : handlers;
}

// Runs `body` with the event's `stopImmediatePropagation` wrapped to raise a
// flag, so handlers behind one host callback stop where the host would have
// stopped separate callbacks. The original is still called.
function guardStop(args: unknown[], body: (stopped: () => boolean) => void): void {
  const event = args[0] as { stopImmediatePropagation?: unknown } | null | undefined;
  if (typeof event !== "object" || event === null) return body(() => false);
  const original = event.stopImmediatePropagation;
  if (typeof original !== "function") return body(() => false);

  const own = Object.hasOwn(event, "stopImmediatePropagation");
  let stopped = false;
  try {
    event.stopImmediatePropagation = (...a: unknown[]): unknown => {
      stopped = true;
      return (original as (...a: unknown[]) => unknown).apply(event, a);
    };
  } catch {
    return body(() => false);
  }
  try {
    body(() => stopped);
  } finally {
    if (own) event.stopImmediatePropagation = original;
    else delete event.stopImmediatePropagation;
  }
}

// A fired `.once` slot stays as a tombstone until its prop key is removed.
// `attachedAt` is the newest `eventId` seen when the slot was created; the
// slot skips that event and every older one.
interface Slot {
  attachedAt: number;
  handlers: EventListener[];
  instance: ComponentInternalInstance | null;
  once: boolean;
  fired: boolean;
}

/**
 * What native holds per element, event, phase and passivity: one dispatcher,
 * registered while any slot is live, running the slots in insertion order.
 * Each slot is keyed by the raw prop name, so `onClick` and `onClickOnce`
 * coexist and patch independently.
 */
interface Entry {
  event: string;
  options: ListenerOptions;
  slots: Map<string, Slot>;
  registered: boolean;
  dispatcher: EventListener;
}

const entriesByEl = new WeakMap<IncaElement, Map<string, Entry>>();

const entryKeyOf = ({ event, capture, passive }: ParsedEventKey): string =>
  `${event}:${capture === true}:${passive === true}`;

/**
 * Brings the host registration in line with whether a live slot exists.
 */
function sync(core: IncaCore, el: IncaElement, entryKey: string, entry: Entry): void {
  const live = Array.from(entry.slots.values()).some((slot) => !slot.fired);
  if (live && !entry.registered && !isDestroyed(el)) {
    core.addEventListener(el.id, entry.event, entry.dispatcher, entry.options);
    entry.registered = true;
  } else if (!live && entry.registered) {
    entry.registered = false;
    core.removeEventListener(el.id, entry.event, entry.dispatcher, entry.options);
  }
  if (entry.slots.size === 0) entriesByEl.get(el)?.delete(entryKey);
}

function createEntry(
  core: IncaCore,
  el: IncaElement,
  entryKey: string,
  { event, capture, passive }: ParsedEventKey,
): Entry {
  const entry: Entry = {
    event,
    options: { capture, passive },
    slots: new Map(),
    registered: false,
    dispatcher: (...args) => {
      const errors: unknown[] = [];
      const eventId = (args[0] as { eventId?: unknown } | null | undefined)?.eventId;
      guardStop(args, (stopped) => {
        // A slot removed by an earlier handler is skipped; one patched keeps its
        // place; one attached during this event waits for the next event.
        for (const key of Array.from(entry.slots.keys())) {
          if (stopped()) break;
          const slot = entry.slots.get(key);
          if (!slot || slot.fired) continue;
          if (typeof eventId === "number" && slot.attachedAt >= eventId) continue;
          // Retire a `.once` slot before it runs, so a synchronous re-dispatch can't re-enter.
          if (slot.once) {
            slot.fired = true;
            sync(core, el, entryKey, entry);
          }
          for (const handler of slot.handlers) {
            if (stopped()) break;
            try {
              callWithAsyncErrorHandling(
                handler,
                slot.instance,
                ErrorCodes.NATIVE_EVENT_HANDLER,
                args,
              );
            } catch (error) {
              errors.push(error);
            }
          }
        }
      });
      // The first error is thrown to the host; each later one is an unhandled rejection.
      for (const error of errors.slice(1)) void Promise.reject(error);
      if (errors.length > 0) throw errors[0];
    },
  };
  return entry;
}

function removeSlot(core: IncaCore, el: IncaElement, parsed: ParsedEventKey, rawKey: string): void {
  const entryKey = entryKeyOf(parsed);
  const entry = entriesByEl.get(el)?.get(entryKey);
  if (!entry?.slots.delete(rawKey)) return;
  sync(core, el, entryKey, entry);
}

function patchEvent(
  core: IncaCore,
  el: IncaElement,
  rawKey: string,
  nextValue: unknown,
  instance: ComponentInternalInstance | null,
): void {
  const parsed = parseEventKey(rawKey);
  const handlers = asHandlers(nextValue);
  if (!handlers) {
    removeSlot(core, el, parsed, rawKey);
    return;
  }

  const entryKey = entryKeyOf(parsed);
  let entries = entriesByEl.get(el);
  if (!entries) entriesByEl.set(el, (entries = new Map()));
  let entry = entries.get(entryKey);
  if (!entry) entries.set(entryKey, (entry = createEntry(core, el, entryKey, parsed)));

  const existing = entry.slots.get(rawKey);
  if (existing) {
    existing.handlers = handlers;
    existing.instance = instance;
    return;
  }

  entry.slots.set(rawKey, {
    attachedAt: core.latestEventId(),
    handlers,
    instance,
    once: parsed.once,
    fired: false,
  });
  try {
    sync(core, el, entryKey, entry);
  } catch (error) {
    entry.slots.delete(rawKey);
    if (entry.slots.size === 0) entries.delete(entryKey);
    throw error;
  }
}

// Props the host reads by presence.
const BOOLEAN_PRESENCE = new Set(["disabled", "hidden", "inert"]);

/**
 * {@link RendererOptions.patchProp} applies one changed `v-bind`/
 * attribute/event prop to a host element.
 *
 * ### `style`
 *
 * An object (per `:style="{...}"`) is diffed against the entries already
 * sent for this element.
 * - `core.setStyle` runs for each entry that is new or changed.
 * - `core.removeStyle` runs for each sent entry that is now absent, `null`,
 *   or not a string/number. After a removal, `core.setStyle` runs again for
 *   every remaining string/number entry.
 * - A string or `null` `style` removes every entry.
 * - A key takes any of three spellings: kebab-case (`flex-direction`),
 *   camelCase (`flexDirection`) or snake_case (`flex_direction`). If an
 *   object names one property in two spellings, the value set most recently
 *   applies.
 *
 * ### `onXxx`
 *
 * The key registers `nextValue` as a listener for `xxx`, taking a function
 * or an array of them, and unbinds that key's listener for anything else.
 * - Order: each distinct key (`onClick`, `onClickOnce`) is its own listener.
 *   Keys that share the event, phase and passivity run in the order first
 *   registered, a re-patch keeps its place, and the host sees one registration
 *   per element, event, phase and passivity. Capture registrations run before
 *   every bubble registration, also on the target. Within a pass,
 *   registrations run in the order the host registered them.
 * - Propagation: `stopImmediatePropagation()` stops the listeners after it.
 * - Timing: a listener attached while an event is being handled first runs
 *   for the next event; events are told apart by the `eventId` the host puts
 *   on each one.
 * - Modifiers: a trailing `Once`/`Passive`/`Capture` suffix (from Vue's
 *   `.once`/`.passive`/`.capture` modifiers) is stripped first. `.once`
 *   removes the listener after it fires once, and a listener that has fired
 *   stays spent until its key is removed. `.capture` registers the listener
 *   for the phase in which the event travels down to the target, and
 *   `.passive` ignores `preventDefault()` calls inside it.
 * - Event names: `onXxx` listens for the hyphenated name (`onMyEvent` listens
 *   for `my-event`), and `on:xxx` listens for the name after the colon as
 *   written (`on:myEvent` listens for `myEvent`). The listener runs when the
 *   host dispatches that name. See {@link EventListener} for the fields an
 *   event carries.
 * - Errors: an error a handler throws, or a rejection of an async handler,
 *   goes to the `onErrorCaptured` hooks and the app's `errorHandler`. Every
 *   handler of an event runs even when an earlier one fails, until one calls
 *   `stopImmediatePropagation()`. An error that no hook or `errorHandler`
 *   takes is logged to the console in a production build, or raised to the
 *   host's error report when `throwUnhandledErrorInProduction` is set. A
 *   development build warns and raises it to the host's error report, once
 *   per failing handler.
 *
 * ### Other props
 *
 * A string, number or boolean `nextValue` goes to `core.setAttribute` as it
 * is. A `disabled`, `hidden` or `inert` set to `false` goes to
 * `core.removeAttribute`, as does a `null` or `undefined` `nextValue`, which
 * includes a prop that is absent from the new vnode. Any other value (an
 * object, array, function or symbol) goes to `core.setAttribute` as
 * `String(nextValue)`. An `onUpdate:` key is a component event and sets
 * nothing.
 *
 * @param core - the incajs bindings to drive the native tree through
 */
export function createPatchProp(
  core: IncaCore,
): RendererOptions<unknown, IncaElement>["patchProp"] {
  return (
    el: IncaElement,
    key: string,
    _prevValue: unknown,
    nextValue: unknown,
    _namespace?: unknown,
    parentComponent?: ComponentInternalInstance | null,
  ): void => {
    if (key === "style") {
      patchStyle(core, el, nextValue);
    } else if (isOn(key)) {
      patchEvent(core, el, key, nextValue, parentComponent ?? null);
    } else if (BOOLEAN_PRESENCE.has(key) && nextValue === false) {
      core.removeAttribute(el.id, key);
    } else if (
      typeof nextValue === "string" ||
      typeof nextValue === "number" ||
      typeof nextValue === "boolean"
    ) {
      core.setAttribute(el.id, key, nextValue);
    } else if (nextValue === null || nextValue === undefined) {
      core.removeAttribute(el.id, key);
    } else if (!key.startsWith("onUpdate:")) {
      // objects stringify as in a browser attribute
      // oxlint-disable-next-line typescript/no-base-to-string
      core.setAttribute(el.id, key, String(nextValue));
    }
  };
}
