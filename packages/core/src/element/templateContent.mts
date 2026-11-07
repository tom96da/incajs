// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The `content` of a template: one fragment handle for each template handle.

import { native } from "../native.mts";
import { releaseWhenCollected, storeContent } from "./lifecycle.mts";
import type { NodeId } from "../types.mts";
import type { IncaElement, IncaFragment } from "./types.mts";

/**
 * Whether a tag names a template, whatever its case.
 * @param tag - the tag or tag name
 * @returns `true` for `template`
 */
export function isTemplateTag(tag: string): boolean {
  return tag.toLowerCase() === "template";
}

/**
 * Gives a new template handle its content handle, so `content` reads the same object before and
 * after the host frees the template. The host holds the template until either handle is
 * collected, and the content handle keeps the template reachable.
 * @param template - the new template handle
 * @param build - builds the handle of a fragment
 * @internal
 */
export function attachContent(template: IncaElement, build: (id: NodeId) => IncaFragment): void {
  storeContent(template, build(native().templateContent(template.__node_id)!));
  native().holdNode(template.__node_id);
  releaseWhenCollected(template);
}
