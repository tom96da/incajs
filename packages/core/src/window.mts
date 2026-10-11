// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createEventTarget } from "./createEventTarget.mts";
import { rootNodeId } from "./tree.mts";

/**
 * The event target at the top of the app. Bubble listeners on it run for
 * every bubbling event, and capture listeners run for every event dispatched
 * in the app, including events that do not bubble. Key events reach it while
 * nothing is focused. A `wheel` listener is passive unless its options say
 * `passive: false`.
 *
 * It has the listener methods `addEventListener`, `removeEventListener` and
 * `dispatchEvent`.
 */
export const window = /* @__PURE__ */ createEventTarget(rootNodeId, ["wheel"]);
