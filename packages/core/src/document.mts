// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { withEventHandlers } from "./element/eventHandlers.mts";
import { activeElement } from "./element/lifecycle.mts";
import { createEventTarget } from "./eventTarget.mts";
import { Document } from "./interfaces.mts";
import type { IncaDocument } from "./types.mts";

/**
 * The event target between the elements and `window`. An event at an element
 * passes through it, and it is the next target after the mount container.
 *
 * It has the listener methods `addEventListener`, `removeEventListener` and
 * `dispatchEvent`, the event handler properties such as `onkeydown`, and
 * `activeElement`. A `wheel` listener is passive unless its options say
 * `passive: false`. It is an instance of the global `Document`.
 */
export const document: IncaDocument = /* @__PURE__ */ Object.defineProperties(
  Object.setPrototypeOf(
    withEventHandlers(createEventTarget("$document", ["wheel"]), () => false),
    Document.prototype,
  ),
  {
    [Symbol.toStringTag]: { value: "Document" },
    activeElement: { get: activeElement },
  },
) as IncaDocument;
