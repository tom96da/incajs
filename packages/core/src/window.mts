// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { windowHandle } from "./element/handle.mts";
import type { Window } from "./types.mts";

/**
 * The event target at the top of the app, after `document`. Every event path ends at it. Bubble
 * listeners on it run for every bubbling event, and capture listeners run for every event
 * dispatched in the app, including events that do not bubble. A key event reaches it through the
 * mount container and `document`. A `wheel` listener is passive unless its options say
 * `passive: false`.
 */
export const window: Window = /* @__PURE__ */ windowHandle();
