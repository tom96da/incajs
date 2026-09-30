// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The package's public type surface.

/**
 * Stable handle to a node in the native retained tree, returned by `createNode`
 * and used in every later call that touches that node.
 */
export type NodeId = number;

/**
 * Id assigned to one registered event listener. Managed internally by
 * `setEventListener`/`removeEventListener`/`destroyNode` — callers never see or
 * pass one directly.
 */
export type CallbackId = number;

/**
 * A value `setAttribute`/`setStyle` can take. Anything else raises a catchable
 * exception rather than being silently coerced.
 */
export type AttributeValue = string | number | boolean;

/**
 * Element kind passed to `createNode`. `"text"` is the only tag with
 * dedicated rendering behavior — it renders its `"value"` attribute followed
 * by its descendants' text. Any other string is a generic styled
 * container; the `string & {}` half of this type keeps `"text"`'s
 * autocomplete while still accepting an arbitrary tag name.
 */
export type TagName = "text" | (string & {});

/**
 * Style properties recognized by `setStyle`'s typed overload. A
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
  /** Overflow on both axes; `"scroll"` (alias `"auto"`) makes the container scroll with the wheel. Overridden per axis by {@link StyleProps.overflow_x}/{@link StyleProps.overflow_y}. */
  overflow?: "visible" | "hidden" | "scroll" | "auto";
  /** Horizontal overflow; wins over {@link StyleProps.overflow}. */
  overflow_x?: "visible" | "hidden" | "scroll" | "auto";
  /** Vertical overflow; wins over {@link StyleProps.overflow}. */
  overflow_y?: "visible" | "hidden" | "scroll" | "auto";
}

/**
 * Signature of a callback registered via `setEventListener`. The
 * native host calls it with one argument, an object with `type` (the
 * event's name), `target`/`currentTarget` (both the {@link NodeId} it fired
 * on), `stopPropagation`/`stopImmediatePropagation`/`preventDefault`
 * methods, plus whatever fields that event kind carries — see
 * https://tom96da.github.io/incajs/reference/events for the full list of
 * wired event names and their fields.
 */
export type EventListener = (...args: unknown[]) => void;
