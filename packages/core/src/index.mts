// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The renderer's value exports live in `rendererCore.mts`.

import "./element/fragment.mts";
import "./timers.mts";

export { document } from "./document.mts";
export { window } from "./window.mts";
export type { RectLike, RectValues } from "./element/geometry.mts";
export type { ScrollIntoViewOptions, ScrollOptions } from "./element/scroll.mts";
export type {
  Attr,
  HTMLCollectionOf,
  IncaElement,
  IncaFragment,
  IncaInputElement,
  IncaNode,
  IncaParent,
  NamedNodeMap,
  NodeListOf,
} from "./element/types.mts";
export type {
  AttributeValue,
  CallbackId,
  EventLike,
  EventListener,
  EventListenerObject,
  EventListenerOrObject,
  IncaCompositionEvent,
  IncaDocument,
  IncaEvent,
  IncaEventMap,
  IncaEventTarget,
  IncaFocusEvent,
  IncaInputEvent,
  IncaKeyboardEvent,
  IncaMouseEvent,
  IncaPointerEvent,
  IncaUIEvent,
  IncaWheelEvent,
  IncaWindow,
  ListenerOptions,
  NodeId,
  Px,
  StyleProps,
  TagName,
} from "./types.mts";
