// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The renderer's value exports live in `rendererCore.mts`.

export { document } from "./document.mts";
export { window } from "./window.mts";
export type { RectLike, RectValues } from "./element/geometry.mts";
export type { ScrollIntoViewOptions, ScrollOptions } from "./element/scroll.mts";
export type { IncaElement, IncaNode } from "./element/types.mts";
export type {
  AttributeValue,
  CallbackId,
  EventLike,
  EventListener,
  EventListenerObject,
  EventListenerOrObject,
  IncaDocument,
  IncaEvent,
  IncaEventMap,
  IncaEventTarget,
  IncaFocusEvent,
  IncaKeyboardEvent,
  IncaMouseEvent,
  IncaPointerEvent,
  IncaUIEvent,
  IncaWheelEvent,
  IncaWindow,
  ListenerOptions,
  NodeId,
  StyleProps,
  TagName,
} from "./types.mts";
