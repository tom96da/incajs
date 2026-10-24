// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { withEventHandlers } from "./element/eventHandlers.mts";
import { createEventTarget } from "./eventTarget.mts";
import type { IncaWindow } from "./types.mts";

/**
 * The event target at the top of the app, after `document`. Every event path
 * ends at it. Bubble listeners on it run for every bubbling event, and capture
 * listeners run for every event dispatched in the app, including events that
 * do not bubble. A key event reaches it through the mount container and
 * `document`. A `wheel` listener is passive unless its options say
 * `passive: false`.
 *
 * It has the listener methods `addEventListener`, `removeEventListener` and
 * `dispatchEvent`, and the event handler properties such as `onkeydown`.
 */
export const window: IncaWindow = /* @__PURE__ */ Object.defineProperty(
  withEventHandlers(createEventTarget("$window", ["wheel"]), () => false),
  Symbol.toStringTag,
  { value: "Window" },
) as IncaWindow;
