// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The package's public type surface.

/** Stable handle to a node in the native retained tree, returned by {@link createNode} and used in every later call that touches that node. */
export type NodeId = number;

/** Id assigned to one registered event listener. Managed internally by {@link setEventListener}/{@link removeEventListener}/{@link destroyNode} — callers never see or pass one directly. */
export type CallbackId = number;

/** A value {@link setAttribute}/{@link setStyle} can take. Anything else raises a catchable exception rather than being silently coerced. */
export type AttributeValue = string | number | boolean;

/**
 * Element kind passed to {@link createNode}. `"text"` is the only tag with
 * dedicated rendering behavior — a leaf that renders its `"value"`
 * attribute's string content. Any other string is a generic styled
 * container; the `string & {}` half of this type keeps `"text"`'s
 * autocomplete while still accepting an arbitrary tag name.
 */
export type TagName = "text" | (string & {});

/**
 * Style properties recognized by {@link setStyle}'s typed overload. A
 * deliberately incomplete, v1 layout/paint vocabulary — an unrecognized key
 * or a value shape that doesn't parse (e.g. a malformed enum string) is
 * ignored by the renderer rather than applied or thrown.
 */
export interface StyleProps {
  /** Layout mode for this node's children. `"none"` skips both layout and rendering for the whole subtree. */
  display?: "flex" | "block" | "grid" | "none";
  /** Main-axis direction for a `"flex"` container. */
  flex_direction?: "row" | "column" | "row_reverse" | "column_reverse";
  /** Main-axis alignment of children within this container. */
  justify_content?: "start" | "end" | "center" | "stretch";
  /** Cross-axis alignment of children within this container. */
  align_items?: "start" | "end" | "center" | "stretch";
  /** Uniform spacing, in px, between children — applied on both axes. */
  gap?: number;
  /** Fixed width in px, or `"auto"` to size to content. */
  width?: number | "auto";
  /** Fixed height in px, or `"auto"` to size to content. */
  height?: number | "auto";
  /** Uniform border width in px, applied to all four sides. */
  border_width?: number;
  /** Fill color, as a hex number (`0xRRGGBB`) or a CSS-style string (`"#rrggbb"`/`"#rgb"`). */
  background?: number | string;
  /** Border color. Accepts the same formats as {@link StyleProps.background}. */
  border_color?: number | string;
  /** Color for this container's own text. Cascades to descendant text leaves, same as {@link StyleProps.text_size}; there's no separate per-leaf text styling. */
  text_color?: number | string;
  /** Uniform corner radius in px, applied to all four corners. */
  corner_radius?: number;
  /** Font size in px for this container's own text. Cascades to descendant text leaves, same as {@link StyleProps.text_color}. */
  text_size?: number;
}

/**
 * Signature of a callback registered via {@link setEventListener}. The
 * native host currently always calls it with exactly one argument, the
 * {@link NodeId} the event fired on — the type stays variadic so a future
 * event kind can add a richer payload without a breaking signature change.
 */
export type EventListener = (...args: unknown[]) => void;
