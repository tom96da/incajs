// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type {
  AddEventListenerOptions as AddEventListenerOptionsType,
  Attr as AttrType,
  CSSStyleDeclaration as CSSStyleDeclarationType,
  CharacterData as CharacterDataType,
  ChildNode as ChildNodeType,
  Comment as CommentType,
  CompositionEvent as CompositionEventType,
  CustomEvent as CustomEventType,
  CustomEventInit as CustomEventInitType,
  DOMRect as DOMRectType,
  DOMRectReadOnly as DOMRectReadOnlyType,
  DOMStringMap as DOMStringMapType,
  DOMTokenList as DOMTokenListType,
  Document as DocumentType,
  DocumentFragment as DocumentFragmentType,
  Element as ElementType,
  Event as EventType,
  EventInit as EventInitType,
  EventListener as EventListenerType,
  EventListenerObject as EventListenerObjectType,
  EventListenerOptions as EventListenerOptionsType,
  EventListenerOrEventListenerObject as EventListenerOrEventListenerObjectType,
  EventTarget as EventTargetType,
  FocusEvent as FocusEventType,
  GlobalEventHandlersEventMap as GlobalEventHandlersEventMapType,
  HTMLCollectionOf as HTMLCollectionOfType,
  HTMLElement as HTMLElementType,
  HTMLInputElement as HTMLInputElementType,
  HTMLTemplateElement as HTMLTemplateElementType,
  InputEvent as InputEventType,
  KeyboardEvent as KeyboardEventType,
  MouseEvent as MouseEventType,
  NamedNodeMap as NamedNodeMapType,
  Node as NodeType,
  NodeListOf as NodeListOfType,
  NonDocumentTypeChildNode as NonDocumentTypeChildNodeType,
  ParentNode as ParentNodeType,
  PointerEvent as PointerEventType,
  ScrollIntoViewOptions as ScrollIntoViewOptionsType,
  ScrollOptions as ScrollOptionsType,
  Text as TextType,
  UIEvent as UIEventType,
  WheelEvent as WheelEventType,
  Window as WindowType,
} from "incajs";

// The host defines these globals before the app runs. This file carries their types.
declare global {
  /** The document of the app. The build adds its import to every module that names it. */
  const document: Document;

  /** The window of the app. The build adds its import to every module that names it. */
  const window: Window;

  /** The constants of `Node`, a number for each node type and document position. */
  interface NodeConstants {
    readonly ELEMENT_NODE: 1;
    readonly ATTRIBUTE_NODE: 2;
    readonly TEXT_NODE: 3;
    readonly CDATA_SECTION_NODE: 4;
    readonly ENTITY_REFERENCE_NODE: 5;
    readonly ENTITY_NODE: 6;
    readonly PROCESSING_INSTRUCTION_NODE: 7;
    readonly COMMENT_NODE: 8;
    readonly DOCUMENT_NODE: 9;
    readonly DOCUMENT_TYPE_NODE: 10;
    readonly DOCUMENT_FRAGMENT_NODE: 11;
    readonly NOTATION_NODE: 12;
    readonly DOCUMENT_POSITION_DISCONNECTED: 1;
    readonly DOCUMENT_POSITION_PRECEDING: 2;
    readonly DOCUMENT_POSITION_FOLLOWING: 4;
    readonly DOCUMENT_POSITION_CONTAINS: 8;
    readonly DOCUMENT_POSITION_CONTAINED_BY: 16;
    readonly DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC: 32;
  }

  // The global names of the types of `incajs`. Each adds no member to the exported type.
  interface EventTarget extends EventTargetType {}
  interface Event extends EventType {}
  interface UIEvent extends UIEventType {}
  interface FocusEvent extends FocusEventType {}
  interface MouseEvent extends MouseEventType {}
  interface PointerEvent extends PointerEventType {}
  interface WheelEvent extends WheelEventType {}
  interface KeyboardEvent extends KeyboardEventType {}
  interface InputEvent extends InputEventType {}
  interface CompositionEvent extends CompositionEventType {}
  interface Node extends NodeType {}
  interface Element extends ElementType {}
  interface HTMLElement extends HTMLElementType {}
  interface HTMLInputElement extends HTMLInputElementType {}
  interface HTMLTemplateElement extends HTMLTemplateElementType {}
  interface Text extends TextType {}
  interface Comment extends CommentType {}
  interface DocumentFragment extends DocumentFragmentType {}
  interface Document extends DocumentType {}
  interface Window extends WindowType {}
  interface ParentNode extends ParentNodeType {}
  interface ChildNode extends ChildNodeType {}
  interface NonDocumentTypeChildNode extends NonDocumentTypeChildNodeType {}
  interface CharacterData extends CharacterDataType {}
  interface Attr extends AttrType {}
  interface CSSStyleDeclaration extends CSSStyleDeclarationType {}
  interface DOMStringMap extends DOMStringMapType {}
  interface DOMTokenList extends DOMTokenListType {}
  interface NamedNodeMap extends NamedNodeMapType {}
  interface ScrollIntoViewOptions extends ScrollIntoViewOptionsType {}
  interface ScrollOptions extends ScrollOptionsType {}
  interface GlobalEventHandlersEventMap extends GlobalEventHandlersEventMapType {}
  interface EventListenerOptions extends EventListenerOptionsType {}
  interface AddEventListenerOptions extends AddEventListenerOptionsType {}
  interface EventInit extends EventInitType {}
  interface DOMRect extends DOMRectType {}
  interface DOMRectReadOnly extends DOMRectReadOnlyType {}
  interface EventListener extends EventListenerType<Event, EventTarget> {}
  interface EventListenerObject extends EventListenerObjectType<Event> {}
  type EventListenerOrEventListenerObject = EventListenerOrEventListenerObjectType<
    Event,
    EventTarget
  >;
  interface CustomEvent<T = any> extends CustomEventType<T> {}
  interface CustomEventInit<T = any> extends CustomEventInitType<T> {}
  interface NodeListOf<T extends Node> extends NodeListOfType<T> {}
  interface HTMLCollectionOf<T extends Element> extends HTMLCollectionOfType<T> {}
  type HTMLDocument = Document;

  /** The interface object of event targets, which a script cannot construct. */
  var EventTarget: {
    readonly prototype: object;
    [Symbol.hasInstance](value: unknown): value is EventTarget;
  };

  /** The interface object of events. */
  var Event: {
    readonly prototype: Omit<Event, "isTrusted">;
    readonly NONE: 0;
    readonly CAPTURING_PHASE: 1;
    readonly AT_TARGET: 2;
    readonly BUBBLING_PHASE: 3;
    new (type: string, eventInitDict?: EventInit): Event;
    [Symbol.hasInstance](value: unknown): value is Event;
  };

  /** The interface object of custom events. */
  var CustomEvent: {
    readonly prototype: Omit<CustomEvent, "isTrusted">;
    new <T = any>(type: string, eventInitDict?: CustomEventInit<T>): CustomEvent<T>;
    [Symbol.hasInstance](value: unknown): value is CustomEvent;
  };

  /** The interface object of nodes, which a script cannot construct. */
  var Node: NodeConstants & {
    readonly prototype: NodeConstants;
    [Symbol.hasInstance](value: unknown): value is Node;
  };

  /** The interface object of text and comments, which a script cannot construct. */
  var CharacterData: {
    readonly prototype: NodeConstants;
    [Symbol.hasInstance](value: unknown): value is CharacterData;
  };

  /** The interface object of text nodes, which a script cannot construct. */
  var Text: {
    readonly prototype: NodeConstants;
    [Symbol.hasInstance](value: unknown): value is Text;
  };

  /** The interface object of comments, which a script cannot construct. */
  var Comment: {
    readonly prototype: NodeConstants;
    [Symbol.hasInstance](value: unknown): value is Comment;
  };

  /** The interface object of elements, which a script cannot construct. */
  var Element: {
    readonly prototype: NodeConstants;
    [Symbol.hasInstance](value: unknown): value is Element;
  };

  /** The interface object of HTML elements, which a script cannot construct. */
  var HTMLElement: {
    readonly prototype: NodeConstants;
    [Symbol.hasInstance](value: unknown): value is HTMLElement;
  };

  /** The interface object of fragments, which a script cannot construct. */
  var DocumentFragment: {
    readonly prototype: NodeConstants;
    [Symbol.hasInstance](value: unknown): value is DocumentFragment;
  };

  /** The interface object of `document`. */
  var Document: {
    readonly prototype: NodeConstants;
    [Symbol.hasInstance](value: unknown): value is Document;
  };

  /** The interface object of the HTML document, which a script cannot construct. */
  var HTMLDocument: {
    readonly prototype: NodeConstants;
    [Symbol.hasInstance](value: unknown): value is Document;
  };

  /** The interface object of `element.dataset`, which scripts cannot construct. */
  var DOMStringMap: { readonly prototype: DOMStringMap };

  /** The interface object of shadow roots, which the host never creates. */
  var ShadowRoot: { readonly prototype: NodeConstants };

  /** The interface object of token lists. It cannot be called or constructed. */
  var DOMTokenList: { readonly prototype: DOMTokenList };

  /** The interface object of the `style` of an element, which a script cannot construct. */
  var CSSStyleDeclaration: { readonly prototype: CSSStyleDeclaration };

  function setTimeout<A extends unknown[]>(
    handler: (...args: A) => void,
    delay?: number,
    ...args: A
  ): number;
  function setInterval<A extends unknown[]>(
    handler: (...args: A) => void,
    delay?: number,
    ...args: A
  ): number;
  function clearTimeout(id?: number): void;
  function clearInterval(id?: number): void;

  type FrameRequestCallback = (time: number) => void;
  function requestAnimationFrame(callback: FrameRequestCallback): number;
  function cancelAnimationFrame(handle: number): void;

  interface PerformanceEntry {
    readonly name: string;
    readonly entryType: string;
    readonly startTime: number;
    readonly duration: number;
    toJSON(): any;
  }
  var PerformanceEntry: { prototype: PerformanceEntry; new (): PerformanceEntry };
  type PerformanceEntryList = PerformanceEntry[];

  interface PerformanceMarkOptions {
    detail?: any;
    startTime?: number;
  }
  interface PerformanceMark extends PerformanceEntry {
    readonly detail: any;
  }
  var PerformanceMark: {
    prototype: PerformanceMark;
    new (markName: string, markOptions?: PerformanceMarkOptions): PerformanceMark;
  };

  interface PerformanceMeasureOptions {
    detail?: any;
    start?: string | number;
    duration?: number;
    end?: string | number;
  }
  interface PerformanceMeasure extends PerformanceEntry {
    readonly detail: any;
  }
  var PerformanceMeasure: { prototype: PerformanceMeasure; new (): PerformanceMeasure };

  interface Performance {
    readonly timeOrigin: number;
    now(): number;
    toJSON(): any;
    mark(markName: string, markOptions?: PerformanceMarkOptions): PerformanceMark;
    measure(
      measureName: string,
      startOrMeasureOptions?: string | PerformanceMeasureOptions,
      endMark?: string,
    ): PerformanceMeasure;
    getEntries(): PerformanceEntryList;
    getEntriesByName(name: string, type?: string): PerformanceEntryList;
    getEntriesByType(type: string): PerformanceEntryList;
    clearMarks(markName?: string): void;
    clearMeasures(measureName?: string): void;
  }
  var Performance: { prototype: Performance; new (): Performance };
  var performance: Performance;

  type AlgorithmIdentifier = string | { name: string };
  type BufferSource = ArrayBufferView | ArrayBuffer;

  interface SubtleCrypto {
    digest(algorithm: AlgorithmIdentifier, data: BufferSource): Promise<ArrayBuffer>;
  }
  var SubtleCrypto: { prototype: SubtleCrypto; new (): SubtleCrypto };

  interface Crypto {
    readonly subtle: SubtleCrypto;
    getRandomValues<T extends ArrayBufferView>(array: T): T;
    randomUUID(): `${string}-${string}-${string}-${string}-${string}`;
  }
  var Crypto: { prototype: Crypto; new (): Crypto };
  var crypto: Crypto;

  interface Console {
    assert(condition?: boolean, ...data: unknown[]): void;
    clear(): void;
    count(label?: string): void;
    countReset(label?: string): void;
    debug(...data: unknown[]): void;
    dir(item?: unknown, options?: unknown): void;
    dirxml(...data: unknown[]): void;
    error(...data: unknown[]): void;
    group(...data: unknown[]): void;
    groupCollapsed(...data: unknown[]): void;
    groupEnd(): void;
    info(...data: unknown[]): void;
    log(...data: unknown[]): void;
    table(tabularData?: unknown, properties?: string[]): void;
    time(label?: string): void;
    timeEnd(label?: string): void;
    timeLog(label?: string, ...data: unknown[]): void;
    trace(...data: unknown[]): void;
    warn(...data: unknown[]): void;
  }
  var console: Console;

  interface TextEncoderEncodeIntoResult {
    read: number;
    written: number;
  }
  interface TextEncoder {
    readonly encoding: string;
    encode(input?: string): Uint8Array<ArrayBuffer>;
    encodeInto(source: string, destination: Uint8Array): TextEncoderEncodeIntoResult;
  }
  var TextEncoder: {
    new (): TextEncoder;
    prototype: TextEncoder;
  };

  type AllowSharedBufferSource = ArrayBufferLike | ArrayBufferView;
  interface TextDecoderOptions {
    fatal?: boolean;
    ignoreBOM?: boolean;
  }
  interface TextDecodeOptions {
    stream?: boolean;
  }
  interface TextDecoder {
    readonly encoding: string;
    readonly fatal: boolean;
    readonly ignoreBOM: boolean;
    decode(input?: AllowSharedBufferSource, options?: TextDecodeOptions): string;
  }
  var TextDecoder: {
    new (label?: string, options?: TextDecoderOptions): TextDecoder;
    prototype: TextDecoder;
  };

  interface URL {
    href: string;
    protocol: string;
    username: string;
    password: string;
    host: string;
    hostname: string;
    port: string;
    pathname: string;
    search: string;
    hash: string;
    readonly origin: string;
    readonly searchParams: URLSearchParams;
    toString(): string;
    toJSON(): string;
  }
  var URL: {
    new (url: string | URL, base?: string | URL): URL;
    prototype: URL;
    canParse(url: string | URL, base?: string | URL): boolean;
    parse(url: string | URL, base?: string | URL): URL | null;
  };

  interface URLSearchParamsIterator<T> extends IteratorObject<T, BuiltinIteratorReturn, unknown> {
    [Symbol.iterator](): URLSearchParamsIterator<T>;
  }
  interface URLSearchParams {
    readonly size: number;
    append(name: string, value: string): void;
    delete(name: string, value?: string): void;
    get(name: string): string | null;
    getAll(name: string): string[];
    has(name: string, value?: string): boolean;
    set(name: string, value: string): void;
    sort(): void;
    forEach(
      callback: (value: string, key: string, parent: URLSearchParams) => void,
      thisArg?: unknown,
    ): void;
    entries(): URLSearchParamsIterator<[string, string]>;
    keys(): URLSearchParamsIterator<string>;
    values(): URLSearchParamsIterator<string>;
    [Symbol.iterator](): URLSearchParamsIterator<[string, string]>;
    toString(): string;
  }
  var URLSearchParams: {
    new (init?: string | string[][] | Record<string, string> | URLSearchParams): URLSearchParams;
    prototype: URLSearchParams;
  };

  interface StructuredSerializeOptions {
    transfer?: ArrayBuffer[];
  }
  function structuredClone<T = any>(value: T, options?: StructuredSerializeOptions): T;
}

// The JavaScript engine provides these globals.
declare global {
  interface DOMException {
    readonly name: string;
    readonly message: string;
    readonly code: number;
    stack?: string;
    toString(): string;
    readonly INDEX_SIZE_ERR: 1;
    readonly DOMSTRING_SIZE_ERR: 2;
    readonly HIERARCHY_REQUEST_ERR: 3;
    readonly WRONG_DOCUMENT_ERR: 4;
    readonly INVALID_CHARACTER_ERR: 5;
    readonly NO_DATA_ALLOWED_ERR: 6;
    readonly NO_MODIFICATION_ALLOWED_ERR: 7;
    readonly NOT_FOUND_ERR: 8;
    readonly NOT_SUPPORTED_ERR: 9;
    readonly INUSE_ATTRIBUTE_ERR: 10;
    readonly INVALID_STATE_ERR: 11;
    readonly SYNTAX_ERR: 12;
    readonly INVALID_MODIFICATION_ERR: 13;
    readonly NAMESPACE_ERR: 14;
    readonly INVALID_ACCESS_ERR: 15;
    readonly VALIDATION_ERR: 16;
    readonly TYPE_MISMATCH_ERR: 17;
    readonly SECURITY_ERR: 18;
    readonly NETWORK_ERR: 19;
    readonly ABORT_ERR: 20;
    readonly URL_MISMATCH_ERR: 21;
    readonly QUOTA_EXCEEDED_ERR: 22;
    readonly TIMEOUT_ERR: 23;
    readonly INVALID_NODE_TYPE_ERR: 24;
    readonly DATA_CLONE_ERR: 25;
  }
  var DOMException: {
    prototype: DOMException;
    new (message?: string, name?: string): DOMException;
    readonly INDEX_SIZE_ERR: 1;
    readonly DOMSTRING_SIZE_ERR: 2;
    readonly HIERARCHY_REQUEST_ERR: 3;
    readonly WRONG_DOCUMENT_ERR: 4;
    readonly INVALID_CHARACTER_ERR: 5;
    readonly NO_DATA_ALLOWED_ERR: 6;
    readonly NO_MODIFICATION_ALLOWED_ERR: 7;
    readonly NOT_FOUND_ERR: 8;
    readonly NOT_SUPPORTED_ERR: 9;
    readonly INUSE_ATTRIBUTE_ERR: 10;
    readonly INVALID_STATE_ERR: 11;
    readonly SYNTAX_ERR: 12;
    readonly INVALID_MODIFICATION_ERR: 13;
    readonly NAMESPACE_ERR: 14;
    readonly INVALID_ACCESS_ERR: 15;
    readonly VALIDATION_ERR: 16;
    readonly TYPE_MISMATCH_ERR: 17;
    readonly SECURITY_ERR: 18;
    readonly NETWORK_ERR: 19;
    readonly ABORT_ERR: 20;
    readonly URL_MISMATCH_ERR: 21;
    readonly QUOTA_EXCEEDED_ERR: 22;
    readonly TIMEOUT_ERR: 23;
    readonly INVALID_NODE_TYPE_ERR: 24;
    readonly DATA_CLONE_ERR: 25;
  };

  /** The signal of an `AbortController`. A script cannot construct one. */
  interface AbortSignal extends EventTarget {
    readonly aborted: boolean;
    readonly reason: any;
    throwIfAborted(): void;
    onabort: ((this: AbortSignal, ev: Event) => any) | null;
  }

  /** The interface object of signals and their static methods. */
  var AbortSignal: {
    prototype: Pick<AbortSignal, "aborted" | "reason" | "throwIfAborted">;
    new (): AbortSignal;
    abort(reason?: any): AbortSignal;
    any(signals: AbortSignal[]): AbortSignal;
    timeout(milliseconds: number): AbortSignal;
  };

  /** A controller that aborts its signal. */
  interface AbortController {
    readonly signal: AbortSignal;
    abort(reason?: any): void;
  }

  /** The interface object of controllers. */
  var AbortController: {
    prototype: AbortController;
    new (): AbortController;
  };
}

export {};
