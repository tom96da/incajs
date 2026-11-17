// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The renderer's value exports live in `rendererCore.mts`.

export { document } from "./document.mts";
export { window } from "./window.mts";
export type { DOMRect, DOMRectReadOnly } from "./element/geometry.mts";
export type {
  CharacterData,
  ChildNode,
  Node,
  NonDocumentTypeChildNode,
  ParentNode,
} from "./element/nodeTypes.mts";
export type { ScrollIntoViewOptions, ScrollOptions } from "./element/scroll.mts";
export type {
  Attr,
  Comment,
  CSSStyleDeclaration,
  DocumentFragment,
  DOMStringMap,
  DOMTokenList,
  Element,
  HTMLCollectionOf,
  HTMLElement,
  HTMLInputElement,
  HTMLTemplateElement,
  NamedNodeMap,
  NodeListOf,
  Text,
} from "./element/types.mts";
export type {
  AddEventListenerOptions,
  AttributeValue,
  CallbackId,
  CompositionEvent,
  CustomEvent,
  CustomEventInit,
  Document,
  Event,
  EventInit,
  EventListener,
  EventListenerObject,
  EventListenerOptions,
  EventListenerOrEventListenerObject,
  EventTarget,
  FocusEvent,
  GlobalEventHandlersEventMap,
  InputEvent,
  KeyboardEvent,
  MouseEvent,
  NodeId,
  PointerEvent,
  Px,
  StyleProps,
  TagName,
  UIEvent,
  WheelEvent,
  Window,
} from "./types.mts";
