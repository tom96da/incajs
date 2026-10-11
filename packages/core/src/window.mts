// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createEventTarget } from "./createEventTarget.mts";
import { rootNodeId } from "./tree.mts";

/**
 * The event target at the top of the app. Listeners on it run for every
 * bubbling event, and for key events while nothing is focused. A `wheel`
 * listener is passive unless its options say `passive: false`.
 *
 * It has the listener methods `addEventListener`, `removeEventListener` and
 * `dispatchEvent`.
 */
export const window = /* @__PURE__ */ createEventTarget(rootNodeId, ["wheel"]);
