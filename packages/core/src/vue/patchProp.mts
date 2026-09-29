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

function patchStyle(core: IncaCore, el: IncaElement, nextValue: unknown): void {
  if (typeof nextValue !== "object" || nextValue === null) return;

  for (const [key, value] of Object.entries(nextValue)) {
    // `setStyle` raises on a non-primitive value, the same way
    // `setAttribute` does — so a value shape it can't take (e.g. an array,
    // for DOM's multi-value CSS properties) is skipped rather than thrown.
    if (typeof value === "string" || typeof value === "number") {
      core.setStyle(el.id, key, value);
    }
  }
}

// `@vue/runtime-core` types an `onXxx` prop as `Function | Function[]`, so
// several handlers can arrive for one event. Vue's own DOM renderer collapses
// them into a single native listener; this does the same.
/**
 * Collapses `value` into one listener. An error a handler throws goes to the
 * app's `errorHandler` and the `onErrorCaptured` hooks; with no
 * `errorHandler`, the first one thrown is also rethrown to the host's error
 * report, and an `onErrorCaptured` hook returning `false` does not suppress
 * that. Only errors thrown synchronously are covered by the host report.
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
        return handler(...a);
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
 * - `style` (an object, per `:style="{...}"`) fans out to one
 *   `core.setStyle` call per entry; an entry whose value isn't a
 *   string/number is skipped.
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
 * There is no native "unset" call, so a prop that's removed entirely (a
 * `null`/`undefined` `nextValue`) is left as-is rather than cleared.
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
