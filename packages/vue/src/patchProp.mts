// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { RendererOptions } from "@vue/runtime-core";

import type { EventListener, IncajsCore } from "./core.mts";
import type { GpjsuiElement } from "./nodeOps.mts";

const isOn = (key: string): boolean => /^on[A-Z]/.test(key);

function patchStyle(core: IncajsCore, el: GpjsuiElement, nextValue: unknown): void {
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
function asListener(value: unknown): EventListener | null {
  if (typeof value === "function") return value as EventListener;
  if (!Array.isArray(value)) return null;

  const listeners = value.filter((entry): entry is EventListener => typeof entry === "function");
  if (listeners.length === 0) return null;
  return (...args: unknown[]) => {
    for (const listener of listeners) listener(...args);
  };
}

function patchEvent(core: IncajsCore, el: GpjsuiElement, rawKey: string, nextValue: unknown): void {
  const event = rawKey.slice(2).toLowerCase();
  const listener = asListener(nextValue);
  if (listener) {
    core.setEventListener(el.id, event, listener);
  } else {
    core.removeEventListener(el.id, event);
  }
}

/**
 * {@link RendererOptions.patchProp} — applies one changed `v-bind`/
 * attribute/event prop to a host element:
 *
 * - `style` (an object, per `:style="{...}"`) fans out to one
 *   `core.setStyle` call per entry; an entry whose value isn't a
 *   string/number is skipped.
 * - An `onXxx` key registers `nextValue` as the listener for `xxx`, taking a
 *   function or an array of them, and unbinds `xxx` for anything else — only
 *   `"click"` is wired to real input by the native host today, other event
 *   names are accepted but never fire.
 * - Everything else falls through to `core.setAttribute`, again skipping
 *   a non-string/number/boolean value rather than passing it through.
 *
 * There is no native "unset" call, so a prop that's removed entirely (a
 * `null`/`undefined` `nextValue`) is left as-is rather than cleared.
 * @param core - the incajs bindings to drive the native tree through
 */
export function createPatchProp(
  core: IncajsCore,
): RendererOptions<unknown, GpjsuiElement>["patchProp"] {
  return (el: GpjsuiElement, key: string, _prevValue: unknown, nextValue: unknown): void => {
    if (key === "style") {
      patchStyle(core, el, nextValue);
    } else if (isOn(key)) {
      patchEvent(core, el, key, nextValue);
    } else if (
      typeof nextValue === "string" ||
      typeof nextValue === "number" ||
      typeof nextValue === "boolean"
    ) {
      core.setAttribute(el.id, key, nextValue);
    }
  };
}
