// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { handleOf } from "./element/handle.mts";
import { window } from "./window.mts";

/**
 * Gives the objects that events carry for target ids: `window` for its tag, the
 * element of a node that has one, the root container's element for the host
 * root, the document for its node, and `null` for any other node. The host
 * calls it at the start of each walk that reaches a listener and on every
 * `dispatchEvent()`, and once more for an event's `relatedTarget` node. Core
 * defines it on each load, so the latest copy of core answers.
 */
const targets: typeof globalThis.__inca_targets__ = (ids) =>
  ids.map((id) => {
    if (id === "$window") return window;
    const node = handleOf(id);
    return node?.kind === "element" || node?.kind === "document" ? node : null;
  });

Object.defineProperty(globalThis, "__inca_targets__", { value: targets, configurable: true });
