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
 * `addEventListener`/`removeEventListener`/`destroyNode`.
 */
export type CallbackId = number;

/**
 * A value `setAttribute`/`setStyle` can take. Any other value raises a catchable
 * exception.
 */
export type AttributeValue = string | number | boolean;

/**
 * Element kind passed to `createNode`. `"text"` is the only tag with
 * dedicated rendering behavior: it renders its `"value"` attribute followed
 * by its descendants' text. Any other string is a generic styled
 * container; the `string & {}` half of this type keeps `"text"`'s
 * autocomplete while still accepting an arbitrary tag name.
 */
export type TagName = "text" | (string & {});

/** Where `scrollIntoView` aligns a node on one axis. */
export type ScrollAlignment = "start" | "center" | "end" | "nearest";

/** One overflow value; `"auto"` is an alias of `"scroll"`. */
export type Overflow = "visible" | "hidden" | "clip" | "scroll" | "auto";

/**
 * Style properties, in their snake_case spelling, that `setStyle`'s typed
 * overload checks. A deliberately incomplete, v1 layout/paint vocabulary. The
 * renderer also reads the camelCase and kebab-case spellings. An unrecognized
 * key or a value shape that doesn't parse (e.g. a malformed enum string) is
 * ignored by the renderer, and the host prints a warning line on stderr for
 * it (an empty string `''` prints none).
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
  /** Uniform spacing, in px (`>= 0`), between children, applied on both axes. */
  gap?: number;
  /** Fixed width in px (`>= 0`), or `"auto"` to size to content. */
  width?: number | "auto";
  /** Fixed height in px (`>= 0`), or `"auto"` to size to content. */
  height?: number | "auto";
  /** Uniform border width in px (`>= 0`), applied to all four sides. */
  border_width?: number;
  /** Fill color, as a hex number (`0xRRGGBB`) or a CSS-style string (`"#rrggbb"`/`"#rgb"`). */
  background?: number | string;
  /** Border color. Accepts the same formats as {@link StyleProps.background}. */
  border_color?: number | string;
  /** Color for this container's own text. Cascades to descendant text leaves, same as {@link StyleProps.text_size}; there's no separate per-leaf text styling. */
  text_color?: number | string;
  /** Uniform corner radius in px (`>= 0`), applied to all four corners. */
  corner_radius?: number;
  /** Font size in px (`>= 0`) for this container's own text. Cascades to descendant text leaves, same as {@link StyleProps.text_color}. */
  text_size?: number;
  /** Overflow on both axes, or `"<x> <y>"` per axis; `"scroll"` (alias `"auto"`) makes the container scroll with the wheel. Overridden per axis by {@link StyleProps.overflow_x}/{@link StyleProps.overflow_y}. */
  overflow?: Overflow | `${Overflow} ${Overflow}`;
  /** Horizontal overflow; wins over {@link StyleProps.overflow}. */
  overflow_x?: Overflow;
  /** Vertical overflow; wins over {@link StyleProps.overflow}. */
  overflow_y?: Overflow;
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
  /** Minimum width in px (`>= 0`), or `"auto"` for the default minimum. */
  min_width?: number | "auto";
  /** Minimum height in px (`>= 0`), or `"auto"` for the default minimum. */
  min_height?: number | "auto";
  /** Maximum width in px (`>= 0`), or `"auto"` for no limit. */
  max_width?: number | "auto";
  /** Maximum height in px (`>= 0`), or `"auto"` for no limit. */
  max_height?: number | "auto";
}

/**
 * An `Event` or `CustomEvent` built in this runtime. The host throws a
 * `TypeError` for any other value.
 */
export interface EventLike {
  readonly type: string;
}

/**
 * How a listener registered via `addEventListener` takes part in an event.
 */
export interface ListenerOptions {
  /** Listen while the event travels from the root down to the target. The default listens while it travels back up. */
  capture?: boolean;
  /** `preventDefault()` calls inside the listener are ignored. */
  passive?: boolean;
  /** The listener is removed before it first runs. */
  once?: boolean;
}

/**
 * The listener methods of an object that receives events, such as the
 * `window` export and a template `ref`. A registration is identified by the
 * event name, the listener and `capture`. `removeEventListener` takes the
 * same listener that `addEventListener` received.
 */
export interface IncaEventTarget {
  /**
   * Registers `listener` for `type`. Registering the same listener for the
   * same `type` and `capture` again changes nothing. A `null` listener is
   * ignored.
   * @param type - the event name (e.g. `"keydown"`)
   * @param listener - a function, called with the event's `currentTarget` as
   * `this`, or an object whose `handleEvent` method is called with the object
   * as `this` when the event fires
   * @param options - a {@link ListenerOptions}, or a boolean for `capture`
   */
  addEventListener<E = IncaEvent>(
    type: string,
    listener: EventListenerOrObject<E> | null,
    options?: boolean | ListenerOptions,
  ): void;

  /**
   * Drops the registration of `listener` for `type` with the same `capture`.
   * A call that matches no registration changes nothing.
   * @param type - the event name
   * @param listener - the listener that was registered
   * @param options - an object with `capture`, or a boolean for `capture`
   */
  removeEventListener<E = IncaEvent>(
    type: string,
    listener: EventListenerOrObject<E> | null,
    options?: boolean | Pick<ListenerOptions, "capture">,
  ): void;

  /**
   * Dispatches an `Event` or `CustomEvent` at the target.
   * @param event - the event to dispatch
   * @returns `false` when a listener canceled a cancelable event, else `true`
   * @throws TypeError when `event` is not an `Event` or `CustomEvent`
   * @throws DOMException named `InvalidStateError` when `event` is being dispatched
   */
  dispatchEvent(event: EventLike): boolean;
}

/**
 * The event object a listener receives. It carries these fields and the
 * fields of its event kind, which this type leaves out. A listener that reads
 * them declares its parameter with them, such as
 * `(event: IncaEvent & { key: string }) => void`.
 *
 * See https://incajs.tom96da.com/reference/events for the event names the
 * host dispatches and their fields.
 */
export interface IncaEvent {
  /** The event's name. */
  readonly type: string;
  /**
   * The element the event is about, or `window`. It is the deepest element
   * under the pointer for pointer events, the nearest common ancestor of the
   * press and the release for `click`, `dblclick` and `auxclick`, and the
   * focused element for key events. With no such element it is `window`.
   */
  // At run time it is null only for a node with no live handle: one made by createNode directly,
  // or a collected handle. An event from a Vue element always has a handle.
  readonly target: IncaEventTarget;
  /** The same object as {@link IncaEvent.target}. */
  readonly srcElement: IncaEventTarget;
  /**
   * The element or `window` whose listener is running, and `null` once the
   * event has been handled. A listener function runs with it as `this`.
   */
  readonly currentTarget: IncaEventTarget | null;
  /** Whether the event travels up from `target`. Fixed per event name. */
  readonly bubbles: boolean;
  /** Whether `preventDefault()` can cancel the event. Fixed per event name. */
  readonly cancelable: boolean;
  /** Fixed per event name. */
  readonly composed: boolean;
  /**
   * 1 on the ancestors of `target` while the event travels down, 2 where
   * `currentTarget` is `target`, 3 on the ancestors while it travels up, and 0
   * once the event has been handled.
   */
  readonly eventPhase: 0 | 1 | 2 | 3;
  /** `true` once a listener called `preventDefault()` on a cancelable event. */
  readonly defaultPrevented: boolean;
  /** `true` for events the host dispatches and `false` for events a script dispatches. */
  readonly isTrusted: boolean;
  /** Milliseconds since the host started. */
  readonly timeStamp: number;
  /** A number that grows with each event and is the same on every node that event reaches. */
  readonly eventId: number;
  /** Assigning `true` while the event is dispatched stops it like `stopPropagation()`. */
  cancelBubble: boolean;
  /** The opposite of `defaultPrevented`. Assigning `false` cancels a cancelable event like `preventDefault()`. */
  returnValue: boolean;
  /** Stops the event from reaching the listeners of later nodes on its path. */
  stopPropagation(): void;
  /** Stops the event from reaching any later listener. */
  stopImmediatePropagation(): void;
  /** Cancels the event when it is cancelable and the listener is not passive. */
  preventDefault(): void;
  /**
   * `target` and its ancestors, ending with `window`, and an empty array once
   * the event has been handled.
   */
  composedPath(): IncaEventTarget[];
}

/**
 * Signature of a callback registered via `addEventListener`. It receives the
 * event, an {@link IncaEvent} unless the parameter declares another type, and
 * runs with the event's `currentTarget` as `this`.
 */
export type EventListener<E = IncaEvent> = (this: IncaEventTarget, event: E) => void;

/**
 * An object whose `handleEvent` method receives the events, called with the
 * object as `this`. It takes the event an {@link EventListener} takes.
 */
export interface EventListenerObject<E = IncaEvent> {
  handleEvent(event: E): void;
}

/**
 * What `addEventListener` accepts as a listener: an {@link EventListener} or
 * an {@link EventListenerObject}.
 */
export type EventListenerOrObject<E = IncaEvent> = EventListener<E> | EventListenerObject<E>;
