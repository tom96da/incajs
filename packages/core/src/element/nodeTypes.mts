// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { IncaElement } from "./types.mts";

/** The sibling members of the nodes that have a parent and are no document type. */
export interface NonDocumentTypeChildNodeMembers {
  /** The closest earlier sibling that is an element, or `null`. */
  readonly previousElementSibling: IncaElement | null;
  /** The closest later sibling that is an element, or `null`. */
  readonly nextElementSibling: IncaElement | null;
}
