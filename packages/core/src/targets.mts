// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { document } from "./document.mts";
import { rootElement } from "./element/handle.mts";
import { elementOf } from "./element/lifecycle.mts";
import { rootNodeId } from "./tree.mts";
import { window } from "./window.mts";

/**
 * Gives the objects that events carry for target ids: `document` and `window`
 * for their tags, the element of a node that has one, the root container's
 * element for the host root, and `null` for any other node. The host calls it
 * at the start of each walk that reaches a listener and on every
 * `dispatchEvent()`, and once more for an event's `relatedTarget` node. Core
 * defines it on each load, so the latest copy of core answers.
 */
const targets: typeof globalThis.__inca_targets__ = (ids) =>
  ids.map((id) => {
    if (id === "$document") return document;
    if (id === "$window") return window;
    return elementOf(id) ?? (id === rootNodeId() ? rootElement() : null);
  });

Object.defineProperty(globalThis, "__inca_targets__", { value: targets, configurable: true });
