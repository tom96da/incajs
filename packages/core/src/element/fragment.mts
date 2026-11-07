// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// `document.createDocumentFragment()`.

import { document } from "../document.mts";
import { createNode, holdNode } from "../tree.mts";
import { createIncaFragment } from "./handle.mts";
import { releaseWhenCollected } from "./lifecycle.mts";
import type { IncaFragment } from "./types.mts";

/**
 * Allocates a fragment and holds it until its handle is collected.
 * @returns the new fragment
 */
function createDocumentFragment(): IncaFragment {
  const fragment = createIncaFragment(createNode("#fragment"));
  holdNode(fragment.__node_id);
  releaseWhenCollected(fragment);
  return fragment;
}

Object.defineProperty(document, "createDocumentFragment", {
  value: createDocumentFragment,
  configurable: true,
  writable: true,
});
