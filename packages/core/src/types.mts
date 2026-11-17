// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The package's public type surface.

import type { ParentNode } from "./element/nodeTypes.mts";
import type {
  Comment,
  DocumentFragment,
  Element,
  EventHandlers,
  HTMLElement,
  Text,
} from "./element/types.mts";
import type { AttributeValue, CallbackId, NodeId, ScrollAlignment } from "./ffi/index.mts";

export type { AttributeValue, CallbackId, NodeId, ScrollAlignment };

/** Element kind passed to `createNode`. */
export type TagName = string;

/**
 * One overflow value. `"scroll"` and `"auto"` scroll with the wheel. The scrollbar of `"auto"`
 * draws over the content, and the one of `"scroll"` reserves its width where the platform does
 * not hide scrollbars.
 */
export type Overflow = "visible" | "hidden" | "clip" | "scroll" | "auto";

/**
 * A size in px: a number, `"0"` or a string such as `"10px"`. A string that reads as zero
 * (`"0.0"`, `"-0"`) is also valid, and the unit is case-insensitive. The host warns on a space
 * before `px`, a trailing `.`, a `.` before the exponent, hexadecimal, binary and octal, which the
 * type admits.
 */
export type Px = number | "0" | `${number}px`;

/**
 * Style properties that `setStyle`'s typed overload checks. A deliberately
 * incomplete, v1 layout/paint vocabulary, written with the CSS property names.
 * An unrecognized key or a value shape that doesn't parse (e.g. a malformed
 * enum string) is ignored by the renderer, and the host prints a warning line
 * on stderr for it (an empty string `''` prints none).
 */
export interface StyleProps {
  /** Layout mode for this node's children. `"none"` skips both layout and rendering for the whole subtree. */
  display?: "flex" | "block" | "grid" | "none";
  /** Main-axis direction for a `"flex"` container. */
  flexDirection?: "row" | "column" | "row-reverse" | "column-reverse";
  /** Main-axis alignment of children within this container. */
  justifyContent?: "start" | "end" | "flex-start" | "flex-end" | "center" | "stretch";
  /** Cross-axis alignment of children within this container. */
  alignItems?: "start" | "end" | "flex-start" | "flex-end" | "center" | "stretch";
  /** Uniform spacing, in px (`>= 0`), between children, applied on both axes. */
  gap?: Px;
  /** Fixed width in px (`>= 0`), or `"auto"` to size to content. */
  width?: Px | "auto";
  /** Fixed height in px (`>= 0`), or `"auto"` to size to content. */
  height?: Px | "auto";
  /** Uniform border width in px (`>= 0`), applied to all four sides. */
  borderWidth?: Px;
  /**
   * Fill color, as a hex number (`0xRRGGBB`) or a color string: `"#rgb"`, `"#rgba"`, `"#rrggbb"`,
   * `"#rrggbbaa"`, `"transparent"`, `"rgb()"` or `"rgba()"`.
   */
  backgroundColor?: number | string;
  /** Border color. Accepts the same formats as {@link StyleProps.backgroundColor}. */
  borderColor?: number | string;
  /**
   * Color for this container's own text. Accepts the same formats as
   * {@link StyleProps.backgroundColor}. Cascades to descendant text leaves, same as
   * {@link StyleProps.fontSize}; there's no separate per-leaf text styling.
   */
  color?: number | string;
  /** Uniform corner radius in px (`>= 0`), applied to all four corners. */
  borderRadius?: Px;
  /** Font size in px (`>= 0`) for this container's own text. Cascades to descendant text leaves, same as {@link StyleProps.color}. */
  fontSize?: Px;
  /**
   * Overflow on both axes, or `"<x> <y>"` per axis; `"scroll"` and `"auto"` make the container
   * scroll with the wheel. Overridden per axis by {@link StyleProps.overflowX}/
   * {@link StyleProps.overflowY}.
   */
  overflow?: Overflow | `${Overflow} ${Overflow}`;
  /** Horizontal overflow; wins over {@link StyleProps.overflow}. */
  overflowX?: Overflow;
  /** Vertical overflow; wins over {@link StyleProps.overflow}. */
  overflowY?: Overflow;
  /** Inner spacing in px (`>= 0`) on all four sides. Overridden per side by the `padding*` side keys. */
  padding?: Px;
  /** Top inner spacing in px (`>= 0`); wins over {@link StyleProps.padding}. */
  paddingTop?: Px;
  /** Right inner spacing in px (`>= 0`); wins over {@link StyleProps.padding}. */
  paddingRight?: Px;
  /** Bottom inner spacing in px (`>= 0`); wins over {@link StyleProps.padding}. */
  paddingBottom?: Px;
  /** Left inner spacing in px (`>= 0`); wins over {@link StyleProps.padding}. */
  paddingLeft?: Px;
  /** Outer spacing in px (negative allowed) or `"auto"` on all four sides. Overridden per side by the `margin*` side keys. */
  margin?: Px | "auto";
  /** Top outer spacing; wins over {@link StyleProps.margin}. */
  marginTop?: Px | "auto";
  /** Right outer spacing; wins over {@link StyleProps.margin}. */
  marginRight?: Px | "auto";
  /** Bottom outer spacing; wins over {@link StyleProps.margin}. */
  marginBottom?: Px | "auto";
  /** Left outer spacing; wins over {@link StyleProps.margin}. */
  marginLeft?: Px | "auto";
  /**
   * Share (`>= 0`) of the free space in the parent's main axis this node takes.
   * The host warns on a trailing `.`, a `.` before the exponent, hexadecimal, binary and octal,
   * which the type admits.
   */
  flexGrow?: number | `${number}`;
  /**
   * Share (`>= 0`) of the overflow in the parent's main axis this node gives up.
   * The host warns on a trailing `.`, a `.` before the exponent, hexadecimal, binary and octal,
   * which the type admits.
   */
  flexShrink?: number | `${number}`;
  /**
   * Opacity from `0` to `1`, or a percentage from `0%` to `100%`, applied to this node and its
   * descendants; values outside are clamped. The host warns on a trailing `.`, a `.` before the
   * exponent, hexadecimal, binary and octal, which the type admits.
   */
  opacity?: number | `${number}` | `${number}%`;
  /** Minimum width in px (`>= 0`), or `"auto"` for the default minimum. */
  minWidth?: Px | "auto";
  /** Minimum height in px (`>= 0`), or `"auto"` for the default minimum. */
  minHeight?: Px | "auto";
  /** Maximum width in px (`>= 0`), or `"auto"` for no limit. */
  maxWidth?: Px | "auto";
  /** Maximum height in px (`>= 0`), or `"auto"` for no limit. */
  maxHeight?: Px | "auto";
}

/**
 * An event built in this runtime with `Event`, `CustomEvent` or another
 * event constructor. The host throws a `TypeError` for any other value.
 */
export interface EventLike {
  readonly type: string;
}

/** The option of `removeEventListener`, which picks the registration to drop. */
export interface EventListenerOptions {
  /**
   * Listen while the event travels from `window` down to the target. The default listens while it
   * travels back up.
   */
  capture?: boolean;
}

/**
 * How a listener registered via `addEventListener` takes part in an event.
 */
export interface AddEventListenerOptions extends EventListenerOptions {
  /** `preventDefault()` calls inside the listener are ignored. */
  passive?: boolean;
  /** The listener is removed before it first runs. */
  once?: boolean;
  /** The listener is removed when the signal aborts. A signal that is aborted already registers nothing. */
  signal?: AbortSignal;
}

/**
 * The listener methods of an object that receives events, such as the
 * `window` and `document` exports and a template `ref`. A registration is identified by the
 * event name, the listener and `capture`. `removeEventListener` takes the
 * same listener that `addEventListener` received.
 */
export interface EventTarget {
  /**
   * Registers `listener` for `type`. Registering the same listener for the
   * same `type` and `capture` again changes nothing. A `null` listener is
   * ignored.
   * @param type - the event name (e.g. `"keydown"`)
   * @param listener - a function, called with the event's `currentTarget` as
   * `this`, or an object whose `handleEvent` method is called with the object
   * as `this` when the event fires
   * @param options - a {@link AddEventListenerOptions}, or a boolean for `capture`
   */
  addEventListener<K extends keyof GlobalEventHandlersEventMap>(
    type: K,
    listener: EventListenerOrEventListenerObject<GlobalEventHandlersEventMap[K], this> | null,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener<E = Event>(
    type: string,
    listener: EventListenerOrEventListenerObject<E, this> | null,
    options?: boolean | AddEventListenerOptions,
  ): void;

  /**
   * Drops the registration of `listener` for `type` with the same `capture`.
   * A call that matches no registration changes nothing.
   * @param type - the event name
   * @param listener - the listener that was registered
   * @param options - an object with `capture`, or a boolean for `capture`
   */
  removeEventListener<K extends keyof GlobalEventHandlersEventMap>(
    type: K,
    listener: EventListenerOrEventListenerObject<GlobalEventHandlersEventMap[K], this> | null,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener<E = Event>(
    type: string,
    listener: EventListenerOrEventListenerObject<E, this> | null,
    options?: boolean | EventListenerOptions,
  ): void;

  /**
   * Dispatches an event built with an event constructor at the target.
   * @param event - the event to dispatch
   * @returns `false` when a listener canceled a cancelable event, else `true`
   * @throws TypeError when `event` is not an event built with an event constructor
   * @throws DOMException named `InvalidStateError` when `event` is being dispatched
   */
  dispatchEvent(event: Event): boolean;
}

/**
 * The top node of the tree, the parent of the mount container. It holds at most one element and
 * any number of comments, and no text.
 */
export interface Document extends ParentNode, EventHandlers<Document> {
  readonly nodeType: 9;
  /** @internal */
  readonly kind: "document";
  /** Always `null`. An assignment does nothing. */
  textContent: string | null;
  /**
   * The element that has focus, or else the element of the document, or `null` when it has none.
   * A `focus()` or `blur()` call takes effect for this property at once.
   */
  readonly activeElement: Element | null;
  /**
   * Creates an empty fragment. Inserting it moves its children into the node it is inserted into.
   * @returns the new fragment
   */
  createDocumentFragment(): DocumentFragment;
  /**
   * Creates an element. The name converts to a string, and its ASCII upper case becomes lower
   * case. The second argument of the web API is ignored.
   * @param localName - The element name, such as `"div"`.
   * @returns The new element, which has no parent.
   * @throws TypeError when called without an argument
   * @throws DOMException named `InvalidCharacterError` when `localName` is not a valid element name
   */
  createElement(localName: string): HTMLElement;
  /**
   * Creates an element with a namespace. The namespace converts to a string, and `null`,
   * `undefined` and `""` mean no namespace. The name converts to a string and splits at its first
   * colon into a prefix and a local name. The element keeps the case of the name.
   * @param namespace - The namespace, such as `"http://www.w3.org/2000/svg"`.
   * @param qualifiedName - The element name, such as `"svg"` or `"x:item"`.
   * @returns The new element, which has no parent.
   * @throws TypeError when called with fewer than two arguments
   * @throws DOMException named `InvalidCharacterError` when the prefix or the local name is not a
   *   valid element name
   * @throws DOMException named `NamespaceError` when the prefix and the namespace do not fit
   */
  createElementNS(namespace: string | null, qualifiedName: string): Element;
  /**
   * Creates a text node.
   * @param data - The text. It converts to a string.
   * @returns The new text node, which has no parent.
   * @throws TypeError when called without an argument
   */
  createTextNode(data: string): Text;
  /**
   * Creates a comment.
   * @param data - The text. It converts to a string.
   * @returns The new comment, which has no parent.
   * @throws TypeError when called without an argument
   */
  createComment(data: string): Comment;
}

/**
 * The event target at the top of every event path.
 */
export interface Window extends EventTarget, EventHandlers<Window> {}

/**
 * The fields every event carries. {@link GlobalEventHandlersEventMap} gives the event type of
 * each name the host dispatches, and a listener for any other name receives
 * this type.
 *
 * See https://incajs.tom96da.com/reference/events for the event names the
 * host dispatches and their fields.
 */
export interface Event {
  /** The event's name. */
  readonly type: string;
  /** `0`, the {@link Event.eventPhase} outside a dispatch. */
  readonly NONE: 0;
  /** `1`, the {@link Event.eventPhase} on an ancestor while the event travels down. */
  readonly CAPTURING_PHASE: 1;
  /** `2`, the {@link Event.eventPhase} on the target. */
  readonly AT_TARGET: 2;
  /** `3`, the {@link Event.eventPhase} on an ancestor while the event travels up. */
  readonly BUBBLING_PHASE: 3;
  /**
   * The element the event is about. It is the deepest element under the pointer for pointer
   * events (the capturing element while the pointer is captured), the nearest common ancestor of
   * the press and the release for `click`, `dblclick` and `auxclick`, and the focused element for
   * key events. With no such element it is the mount container's element, or `document` while
   * the mount container is out of the document. A script dispatch at `document` or `window`
   * gives that object.
   */
  // At run time it is null for an event that no dispatch has reached, and for a node with no live
  // handle, as one made by createNode directly or a collected one.
  readonly target: EventTarget | null;
  /** The same object as {@link Event.target}. */
  readonly srcElement: EventTarget | null;
  /**
   * The element, `document` or `window` whose listener is running, and `null` once the
   * event has been handled. A listener function runs with it as `this`.
   */
  readonly currentTarget: EventTarget | null;
  /**
   * Whether the event travels up from `target`. Fixed per event name for the
   * events the host dispatches.
   */
  readonly bubbles: boolean;
  /**
   * Whether `preventDefault()` can cancel the event. Fixed per event name for
   * the events the host dispatches.
   */
  readonly cancelable: boolean;
  /** Fixed per event name for the events the host dispatches. */
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
  /** Milliseconds since the app was loaded. */
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
   * `target` and its ancestors, then `document` and `window`, and an empty
   * array once the event has been handled.
   */
  composedPath(): EventTarget[];
  /**
   * Sets the type, `bubbles` and `cancelable` of an event that is not being
   * dispatched, clears its stop and cancel flags and makes it untrusted. While
   * the event is dispatched it does nothing.
   */
  initEvent(type: string, bubbles?: boolean, cancelable?: boolean): void;
}

/** The fields an event constructor takes. */
export interface EventInit {
  bubbles?: boolean;
  cancelable?: boolean;
  composed?: boolean;
}

/** The fields the `CustomEvent` constructor takes. */
export interface CustomEventInit<T = any> extends EventInit {
  detail?: T;
}

/** An event that carries a value of the script's choice in `detail`. */
export interface CustomEvent<T = any> extends Event {
  /** The value the constructor's `detail` field gave, or `null`. */
  readonly detail: T;
  /** Sets the type, `bubbles`, `cancelable` and `detail`. */
  initCustomEvent(type: string, bubbles?: boolean, cancelable?: boolean, detail?: T): void;
}

/**
 * The event object of a mouse, wheel, keyboard, focus, input or composition event.
 */
export interface UIEvent extends Event {
  /**
   * On the events the host dispatches, the click count of `mousedown`,
   * `mouseup`, `click`, `dblclick` and `auxclick`, and `0` on the others.
   */
  readonly detail: number;
  /** Always `null`. */
  readonly view: Window | null;
  /**
   * On the events the host dispatches: for `mousedown`, `mouseup`, `click`,
   * `dblclick`, `auxclick` and `contextmenu`, `button + 1`. For `wheel`, `0`.
   * For the other mouse events, the lowest held button plus 1, or `0` with no
   * button held. For the pointer events, `button + 1`, so `0` when `button` is `-1`. For a
   * keyboard event, `keyCode`. For a focus event, `0`.
   */
  readonly which: number;
}

/** The event object of `focus`, `blur`, `focusin` and `focusout`. */
export interface FocusEvent extends UIEvent {
  /**
   * The element gaining focus for `blur` and `focusout`, the element losing
   * it for `focus` and `focusin`, or `null`.
   */
  readonly relatedTarget: EventTarget | null;
}

/** The event object of the mouse events. */
export interface MouseEvent extends UIEvent {
  readonly screenX: number;
  readonly screenY: number;
  /** The pointer's position in the window. */
  readonly clientX: number;
  /** The pointer's position in the window. */
  readonly clientY: number;
  /** The same as {@link MouseEvent.clientX}. */
  readonly pageX: number;
  /** The same as {@link MouseEvent.clientY}. */
  readonly pageY: number;
  /** The same as {@link MouseEvent.clientX}. */
  readonly x: number;
  /** The same as {@link MouseEvent.clientY}. */
  readonly y: number;
  /** On the events the host dispatches, the pointer's position from the padding edge of `target`. */
  readonly offsetX: number;
  /** On the events the host dispatches, the pointer's position from the padding edge of `target`. */
  readonly offsetY: number;
  /** On the events the host dispatches, the pointer's travel since the previous `mousemove`, and `0` on other events. */
  readonly movementX: number;
  /** On the events the host dispatches, the pointer's travel since the previous `mousemove`, and `0` on other events. */
  readonly movementY: number;
  /**
   * The button the event is about: `0` left, `1` middle, `2` right, `3` back, `4` forward. `-1` on
   * a pointer event that is about no button.
   */
  readonly button: number;
  /** Every button held, as a bitmask: `1` left, `2` right, `4` middle, `8` back, `16` forward. */
  readonly buttons: number;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
  readonly metaKey: boolean;
  /**
   * The element the pointer came from for `mouseover` and `mouseenter`, the
   * element it went to for `mouseout` and `mouseleave`, or `null`.
   */
  readonly relatedTarget: EventTarget | null;
  /**
   * Whether the modifier `key` is held: `"Control"`, `"Shift"`, `"Alt"`,
   * `"Meta"`, or `"Accel"` (Meta on macOS, Control elsewhere). Any other name
   * gives `false`.
   */
  getModifierState(key: string): boolean;
}

/**
 * The event object of the `pointer*` events, `gotpointercapture`, `lostpointercapture`, `click`,
 * `auxclick` and `contextmenu`.
 */
export interface PointerEvent extends MouseEvent {
  /**
   * On the events the host dispatches, `1` for the mouse and for the `contextmenu` of a key, and
   * `-1` for the click of a key.
   */
  readonly pointerId: number;
  readonly width: number;
  readonly height: number;
  /** On the pointer events the host dispatches, `0.5` while a button is held, else `0`. */
  readonly pressure: number;
  /** On the events the host dispatches, `"mouse"`, or `""` for the click of a key. */
  readonly pointerType: string;
  /**
   * On the events the host dispatches, `true` on the pointer events and `false` on `click`,
   * `auxclick` and `contextmenu`.
   */
  readonly isPrimary: boolean;
}

/** The event object of `wheel`. */
export interface WheelEvent extends MouseEvent {
  readonly deltaX: number;
  readonly deltaY: number;
  /** `0` on the events the host dispatches. */
  readonly deltaZ: number;
  /** `0` for pixels, `1` for lines. */
  readonly deltaMode: number;
  readonly DOM_DELTA_PIXEL: 0;
  readonly DOM_DELTA_LINE: 1;
  readonly DOM_DELTA_PAGE: 2;
}

/** The event object of `keydown`, `keypress` and `keyup`. */
export interface KeyboardEvent extends UIEvent {
  /** The key's name, such as `"Enter"`, `"F5"` or the typed character. */
  readonly key: string;
  /** `""` on the events the host dispatches. */
  readonly code: string;
  /** `0` on the events the host dispatches. */
  readonly location: number;
  /** Whether the key press is an auto-repeat. */
  readonly repeat: boolean;
  /** Whether an input method composes in the focused `<input>`. */
  readonly isComposing: boolean;
  /**
   * The key's code on a US keyboard layout, and `0` for a key outside it. On `keypress` it is the
   * `charCode`.
   */
  readonly keyCode: number;
  /** The code of the typed character on `keypress`, and `0` on `keydown` and `keyup`. */
  readonly charCode: number;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
  readonly metaKey: boolean;
  readonly DOM_KEY_LOCATION_STANDARD: 0;
  readonly DOM_KEY_LOCATION_LEFT: 1;
  readonly DOM_KEY_LOCATION_RIGHT: 2;
  readonly DOM_KEY_LOCATION_NUMPAD: 3;
  /**
   * Whether the modifier `key` is held: `"Control"`, `"Shift"`, `"Alt"`,
   * `"Meta"`, or `"Accel"` (Meta on macOS, Control elsewhere). Any other name
   * gives `false`.
   */
  getModifierState(key: string): boolean;
}

/** The event object of `beforeinput` and `input`. */
export interface InputEvent extends UIEvent {
  /**
   * The inserted text, or `null` for a deletion and a line break. On `input`, `null` for an empty
   * composition text too.
   */
  readonly data: string | null;
  /**
   * On the events the host dispatches, one of `insertText`, `insertFromPaste`,
   * `insertCompositionText`, `insertLineBreak`, `deleteContentBackward`, `deleteContentForward`
   * and `deleteByCut`.
   */
  readonly inputType: string;
  /** Whether the edit belongs to a composition. */
  readonly isComposing: boolean;
  /** Always `null`. */
  readonly dataTransfer: null;
  /** Returns a new empty array. */
  getTargetRanges(): [];
}

/** The event object of `compositionstart`, `compositionupdate` and `compositionend`. */
export interface CompositionEvent extends UIEvent {
  /** The composed text. For `compositionstart`, the text the composition replaces. */
  readonly data: string;
}

/** The event object of each event name the host dispatches. */
export interface GlobalEventHandlersEventMap {
  auxclick: PointerEvent;
  beforeinput: InputEvent;
  blur: FocusEvent;
  change: Event;
  click: PointerEvent;
  compositionend: CompositionEvent;
  compositionstart: CompositionEvent;
  compositionupdate: CompositionEvent;
  contextmenu: PointerEvent;
  dblclick: MouseEvent;
  focus: FocusEvent;
  focusin: FocusEvent;
  focusout: FocusEvent;
  gotpointercapture: PointerEvent;
  input: InputEvent;
  keydown: KeyboardEvent;
  keypress: KeyboardEvent;
  keyup: KeyboardEvent;
  lostpointercapture: PointerEvent;
  mousedown: MouseEvent;
  mouseenter: MouseEvent;
  mouseleave: MouseEvent;
  mousemove: MouseEvent;
  mouseout: MouseEvent;
  mouseover: MouseEvent;
  mouseup: MouseEvent;
  pointerdown: PointerEvent;
  pointerenter: PointerEvent;
  pointerleave: PointerEvent;
  pointermove: PointerEvent;
  pointerout: PointerEvent;
  pointerover: PointerEvent;
  pointerup: PointerEvent;
  scroll: Event;
  wheel: WheelEvent;
}

/**
 * Signature of a callback registered via `addEventListener`. It receives the
 * event, an {@link Event} unless the parameter declares another type, and
 * runs with the event's `currentTarget`, the target it was registered on, as
 * `this`.
 */
export interface EventListener<E = Event, T = EventTarget> {
  (this: T, event: E): void;
}

/**
 * An object whose `handleEvent` method receives the events, called with the
 * object as `this`. It takes the event an {@link EventListener} takes.
 */
export interface EventListenerObject<E = Event> {
  handleEvent(event: E): void;
}

/**
 * What `addEventListener` accepts as a listener: an {@link EventListener} or
 * an {@link EventListenerObject}.
 */
export type EventListenerOrEventListenerObject<E = Event, T = EventTarget> =
  | EventListener<E, T>
  | EventListenerObject<E>;
