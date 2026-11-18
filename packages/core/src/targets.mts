// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { handleOf } from "./element/handle.mts";
import { windowTargetId } from "./ffi/index.mts";
import { window } from "./window.mts";

/**
 * Gives the objects that events carry for target ids: `window` for its target, the
 * handle of a node, built when none is live, the root container's element for the host
 * root, the document for its node, and `null` for an id the host no longer has. The host
 * calls it at the start of each walk that reaches a listener and on every
 * `dispatchEvent()`, and once more for an event's `relatedTarget` node. Core
 * defines it on each load, so the latest copy of core answers.
 */
const targets: typeof globalThis.__inca_targets__ = (ids) =>
  ids.map((id) =>
    typeof id === "number" ? (handleOf(id) ?? null) : id === windowTargetId() ? window : null,
  );

Object.defineProperty(globalThis, "__inca_targets__", { value: targets, configurable: true });
