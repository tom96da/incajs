// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { documentHandle } from "./element/handle.mts";
import type { IncaDocument } from "./types.mts";

/**
 * The top node of the tree and the parent of the mount container. An event at an element passes
 * through it, and it is the next target after the mount container.
 *
 * It has the node members, the listener methods `addEventListener`, `removeEventListener` and
 * `dispatchEvent`, the event handler properties such as `onkeydown`, `activeElement`,
 * `createElement()`, `createTextNode()`, `createComment()`, `createDocumentFragment()`, and the
 * query methods `querySelector()` and `querySelectorAll()`. A `wheel` listener is passive unless
 * its options say `passive: false`. It is an instance of the global `Document`.
 */
export const document: IncaDocument = /* @__PURE__ */ documentHandle();
