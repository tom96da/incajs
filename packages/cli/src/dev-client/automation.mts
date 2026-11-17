// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The requests of an automation-feature host, imported by the repository's tests only.

import type { HostClient } from "./client.mts";

/** A box in logical pixels. */
export interface SnapshotBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A node of the window's tree. */
export interface SnapshotNode {
  /** The node's id, the one `click` takes. */
  id: number;
  /** The tag name, `#text` for a text node. */
  tag: string;
  /** The content of a text node, `null` for every other node. */
  text: string | null;
  /** The node's attributes. */
  attributes: Record<string, string | number | boolean>;
  /**
   * The border box of the last drawn frame in window coordinates, `null` for a text node and for
   * a node that frame did not draw.
   */
  bounds: SnapshotBounds | null;
  /** The child nodes in order. */
  children: SnapshotNode[];
}

/**
 * Clicks the center of a node's box, or of its nearest ancestor with a box.
 *
 * @param host - the client of a host built with the automation feature
 * @param node - the id of the node
 * @param timeoutMs - how long to wait for the answer
 * @returns resolves once the click has run
 */
export async function click(host: HostClient, node: number, timeoutMs?: number): Promise<void> {
  await host.call("click", { node }, timeoutMs);
}

/**
 * Reads the window's node tree.
 *
 * @param host - the client of a host built with the automation feature
 * @param timeoutMs - how long to wait for the answer
 * @returns the root node
 */
export async function snapshot(host: HostClient, timeoutMs?: number): Promise<SnapshotNode> {
  return (await host.call("snapshot", undefined, timeoutMs)) as SnapshotNode;
}

/**
 * Runs the queued jobs and frame callbacks, then draws a frame.
 *
 * @param host - the client of a host built with the automation feature
 * @param timeoutMs - how long to wait for the answer
 * @returns resolves once the frame is drawn
 */
export async function waitIdle(host: HostClient, timeoutMs?: number): Promise<void> {
  await host.call("waitIdle", undefined, timeoutMs);
}
