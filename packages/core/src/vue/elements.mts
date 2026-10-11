// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { NodeId } from "../rendererCore.mts";
import type { IncaElement } from "./nodeOps.mts";

/**
 * Every live element handle, keyed by its native node id. Entries leave when
 * Vue removes the node or replaces the text of its parent.
 */
export const elements = new Map<NodeId, IncaElement>();
