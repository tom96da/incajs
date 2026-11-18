// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { native } from "./native.mts";
import type { NodeId, ScrollAlignment } from "../types.mts";
import type { NodeBox, NodeScroll } from "./native.mts";

/**
 * Returns the box `nodeId` was drawn at in the last frame, in window logical
 * pixels, or `null` if the node has no recorded box. A node that is not connected has none,
 * whatever the last frame recorded, and neither has an id that names no node.
 * @param nodeId - the node to read
 * @returns the box, or `null`
 */
export function layoutOf(nodeId: NodeId): NodeBox | null {
  return native().layoutOf(nodeId);
}

/**
 * Returns the scroll offset of `nodeId` in window logical pixels. A write made
 * since the last draw is included, clamped to the maximum of the last drawn
 * frame.
 * @param nodeId - the node to read
 * @returns `[left, top, maxLeft, maxTop]`, or `null` for a node whose `overflow` is
 *   `visible` or `clip` on both axes. The maxima are the largest offsets the node can take, as
 *   of the last drawn frame.
 * @throws {TypeError} when `nodeId` names no node
 */
export function scrollOf(nodeId: NodeId): NodeScroll | null {
  return native().scrollOf(nodeId);
}

/**
 * Sets the scroll offset of `nodeId` on one or both axes. A `null` axis keeps
 * its current offset. The host clamps each value to the scrollable range on
 * the next frame. A node whose `overflow` is `visible` or `clip` on both axes ignores the call.
 * @param nodeId - the node to scroll
 * @param left - the horizontal offset, or `null` to keep it
 * @param top - the vertical offset, or `null` to keep it
 * @throws {TypeError} when `nodeId` names no node
 */
export function scrollNode(nodeId: NodeId, left: number | null, top: number | null): void {
  native().scrollNode(nodeId, left, top);
}

/**
 * Scrolls each scroll container around `nodeId` so the node shows.
 * @param nodeId - the node to bring into view
 * @param block - the vertical alignment: `"start"`, `"center"`, `"end"` or `"nearest"`
 * @param inline - the horizontal alignment: `"start"`, `"center"`, `"end"` or `"nearest"`
 * @throws {TypeError} when `nodeId` names no node
 * @throws {TypeError} when `block` or `inline` is not an alignment name
 */
export function scrollIntoViewNode(
  nodeId: NodeId,
  block: ScrollAlignment,
  inline: ScrollAlignment,
): void {
  native().scrollIntoViewNode(nodeId, block, inline);
}
