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
  /** Inner spacing in px (`>= 0`) on all four sides. Overridden per axis by {@link StyleProps.padding_x}/{@link StyleProps.padding_y} and per side by the `padding_*` side keys. */
  padding?: number;
  /** Horizontal inner spacing in px (`>= 0`); wins over {@link StyleProps.padding}. */
  padding_x?: number;
  /** Vertical inner spacing in px (`>= 0`); wins over {@link StyleProps.padding}. */
  padding_y?: number;
  /** Top inner spacing in px (`>= 0`); wins over {@link StyleProps.padding_y}. */
  padding_top?: number;
  /** Right inner spacing in px (`>= 0`); wins over {@link StyleProps.padding_x}. */
  padding_right?: number;
  /** Bottom inner spacing in px (`>= 0`); wins over {@link StyleProps.padding_y}. */
  padding_bottom?: number;
  /** Left inner spacing in px (`>= 0`); wins over {@link StyleProps.padding_x}. */
  padding_left?: number;
  /** Outer spacing in px (negative allowed) or `"auto"` on all four sides. Overridden per axis by {@link StyleProps.margin_x}/{@link StyleProps.margin_y} and per side by the `margin_*` side keys. */
  margin?: number | "auto";
  /** Horizontal outer spacing; wins over {@link StyleProps.margin}. */
  margin_x?: number | "auto";
  /** Vertical outer spacing; wins over {@link StyleProps.margin}. */
  margin_y?: number | "auto";
  /** Top outer spacing; wins over {@link StyleProps.margin_y}. */
  margin_top?: number | "auto";
  /** Right outer spacing; wins over {@link StyleProps.margin_x}. */
  margin_right?: number | "auto";
  /** Bottom outer spacing; wins over {@link StyleProps.margin_y}. */
  margin_bottom?: number | "auto";
  /** Left outer spacing; wins over {@link StyleProps.margin_x}. */
  margin_left?: number | "auto";
  /** Share (`>= 0`) of the free space in the parent's main axis this node takes. */
  flex_grow?: number;
  /** Share (`>= 0`) of the overflow in the parent's main axis this node gives up. */
  flex_shrink?: number;
  /** Opacity from `0` to `1`, applied to this node and its descendants; values outside are clamped. */
  opacity?: number;
  /** Minimum width in px, or `"auto"` for the default minimum. */
  min_width?: number | "auto";
  /** Minimum height in px, or `"auto"` for the default minimum. */
  min_height?: number | "auto";
  /** Maximum width in px, or `"auto"` for no limit. */
  max_width?: number | "auto";
  /** Maximum height in px, or `"auto"` for no limit. */
  max_height?: number | "auto";
}

/**
 * Signature of a callback registered via `setEventListener`. The
 * native host calls it with one argument, an object with `type` (the
 * event's name), `target`/`currentTarget` (both the {@link NodeId} it fired
 * on), `stopPropagation`/`stopImmediatePropagation`/`preventDefault`
 * methods, plus whatever fields that event kind carries — see
 * https://incajs.tom96da.com/reference/events for the full list of
 * wired event names and their fields.
 */
export type EventListener = (...args: unknown[]) => void;
