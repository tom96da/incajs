// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  callWithAsyncErrorHandling,
  ErrorCodes,
  type ComponentInternalInstance,
  type RendererOptions,
} from "@vue/runtime-core";

import type { IncaCore } from "../rendererCore.mts";
import type { EventListener } from "../types.mts";
import type { IncaElement } from "./nodeOps.mts";

const isOn = (key: string): boolean => /^on[A-Z]/.test(key);

// Vue's SFC compiler appends these to the prop name per event modifier,
// e.g. `@click.once` becomes `onClickOnce`; `@click.once.capture` becomes
// `onClickOnceCapture`. Matches `@vue/runtime-dom`'s own `patchEvent.ts`.
const modifierSuffixRE = /(?:Once|Passive|Capture)$/;

interface ParsedEventKey {
  event: string;
  once: boolean;
}

// Strips modifier suffixes off `rawKey`, repeatedly since Vue can combine
// several, then lower-cases what's left into the native event name.
// `.passive`/`.capture` are recognized but have no native counterpart yet —
// accepted and bound as an ordinary bubble listener rather than left to
// register a dead event name.
function parseEventKey(rawKey: string): ParsedEventKey {
  let key = rawKey;
  let once = false;
  let match: RegExpMatchArray | null;
  while ((match = key.match(modifierSuffixRE))) {
    key = key.slice(0, -match[0].length);
    if (match[0] === "Once") once = true;
  }
  return { event: key.slice(2).toLowerCase(), once };
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

  for (const key of sent.keys()) {
    if (!isSendable(next[key])) {
      core.removeStyle(el.id, key);
      sent.delete(key);
    }
  }
  for (const [key, value] of Object.entries(next)) {
    if (isSendable(value) && sent.get(key) !== value) {
      core.setStyle(el.id, key, value);
      sent.set(key, value);
    }
  }
}

// The bundler replaces `process.env.NODE_ENV`; QuickJS has no `process`.
const isProduction = ((): boolean => {
  try {
    return process.env.NODE_ENV === "production";
  } catch {
    return false;
  }
})();

const isThenable = (value: unknown): value is PromiseLike<unknown> =>
  value != null && typeof (value as PromiseLike<unknown>).then === "function";

// `@vue/runtime-core` types an `onXxx` prop as `Function | Function[]`, so
// several handlers can arrive for one event. Vue's own DOM renderer collapses
// them into a single native listener; this does the same.
/**
 * Collapses `value` into one listener. An error a handler throws goes to the
 * app's `errorHandler` and the `onErrorCaptured` hooks; with no
 * `errorHandler`, the first one thrown is also rethrown to the host's error
 * report, and an `onErrorCaptured` hook returning `false` does not suppress
 * that. A rejected promise from an async handler follows the same route.
 */
function asListener(
  value: unknown,
  instance: ComponentInternalInstance | null,
): EventListener | null {
  const handlers = (Array.isArray(value) ? value : [value]).filter(
    (entry): entry is EventListener => typeof entry === "function",
  );
  if (handlers.length === 0) return null;

  return (...args: unknown[]) => {
    let failure: { error: unknown } | undefined;
    const recorded = handlers.map((handler) => (...a: unknown[]) => {
      try {
        const result: unknown = handler(...a);
        // A production Vue swallows a rejection; keep one unhandled for the host.
        if (isProduction && isThenable(result)) {
          result.then(undefined, (error: unknown) => {
            if (!instance?.appContext.config.errorHandler) throw error;
          });
        }
        return result;
      } catch (error) {
        failure ??= { error };
        throw error;
      }
    });
    callWithAsyncErrorHandling(recorded, instance, ErrorCodes.NATIVE_EVENT_HANDLER, args);
    if (failure && !instance?.appContext.config.errorHandler) throw failure.error;
  };
}

function patchEvent(
  core: IncaCore,
  el: IncaElement,
  rawKey: string,
  nextValue: unknown,
  instance: ComponentInternalInstance | null,
): void {
  const { event, once } = parseEventKey(rawKey);
  const listener = asListener(nextValue, instance);
  if (!listener) {
    core.removeEventListener(el.id, event);
    return;
  }

  if (!once) {
    core.setEventListener(el.id, event, listener);
    return;
  }

  // Real once-semantics: unbind before running the listener, so a
  // synchronous re-dispatch from inside it can't re-enter.
  const runOnce: EventListener = (...args) => {
    core.removeEventListener(el.id, event);
    listener(...args);
  };
  core.setEventListener(el.id, event, runOnce);
}

/**
 * {@link RendererOptions.patchProp} — applies one changed `v-bind`/
 * attribute/event prop to a host element:
 *
 * - `style` (an object, per `:style="{...}"`) is diffed against the entries
 *   already sent for this element. `core.setStyle` runs for each entry that
 *   is new or changed. `core.removeStyle` runs for each sent entry that is now
 *   absent, `null`, or not a string/number. A string or `null` `style`
 *   removes every entry.
 * - An `onXxx` key registers `nextValue` as the listener for `xxx`, taking a
 *   function or an array of them, and unbinds `xxx` for anything else. A
 *   trailing `Once`/`Passive`/`Capture` suffix (from Vue's `.once`/
 *   `.passive`/`.capture` modifiers) is stripped first; `.once` really
 *   removes the listener after it fires once, while `.passive`/`.capture`
 *   bind as an ordinary listener with no native passive/capture-phase
 *   support yet. See {@link EventListener} for which event names are wired
 *   to real input by the native host today; other names are accepted but
 *   never fire. An error a handler throws synchronously goes to the app's
 *   `errorHandler` and `onErrorCaptured` hooks; with no `errorHandler` it is
 *   also rethrown to the host's error report.
 * - Everything else falls through to `core.setAttribute`, again skipping
 *   a non-string/number/boolean value rather than passing it through.
 *
 * Removing any other prop (a `null`/`undefined` `nextValue`) leaves it as-is.
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
    } else if (
      typeof nextValue === "string" ||
      typeof nextValue === "number" ||
      typeof nextValue === "boolean"
    ) {
      core.setAttribute(el.id, key, nextValue);
    }
  };
}
