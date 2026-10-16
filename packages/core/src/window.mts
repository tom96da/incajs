// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createEventTarget } from "./eventTarget.mts";
import { rootNodeId } from "./tree.mts";
import type { IncaEventTarget } from "./types.mts";

/**
 * The event target at the top of the app. Every event path ends at it.
 * Bubble listeners on it run for every bubbling event, and capture listeners
 * run for every event dispatched in the app, including events that do not
 * bubble. Key events reach it while nothing is focused, with `window` as
 * `target`. A `wheel` listener is passive unless its options say
 * `passive: false`.
 *
 * It has the listener methods `addEventListener`, `removeEventListener` and
 * `dispatchEvent`.
 */
export const window: IncaEventTarget = /* @__PURE__ */ Object.defineProperty(
  createEventTarget(rootNodeId, ["wheel"]),
  Symbol.toStringTag,
  { value: "Window" },
);
