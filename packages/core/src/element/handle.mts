// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createEventTarget } from "../eventTarget.mts";
import {
  HTML_NAMESPACE,
  createNode,
  documentNodeId,
  inputValue,
  rootNodeId,
  setAttribute,
  setInputValue,
  setStyle,
  windowTargetId,
} from "../ffi/index.mts";
import { installInterfaces, defineMembers, windowHandlerMembers } from "./interfaces.mts";
import {
  elementOf,
  existingContent,
  isDestroyed,
  nodeOf,
  prototypeOf,
  registerNode,
  registerRoot,
  stateOf,
  withState,
  NO_EVENTS,
  HTML_ELEMENT_RULE,
  receiverNode,
} from "./lifecycle.mts";
import { replaceWithText } from "./nodeTree.mts";
import { attachContent } from "./templateContent.mts";
import { createIncaText } from "./text.mts";
import type { ElementName } from "../ffi/index.mts";
import type { IncaDocument, IncaWindow, NodeId, TagName } from "../types.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type {
  IncaComment,
  IncaElement,
  IncaFragment,
  IncaHTMLElement,
  IncaNode,
  IncaParent,
  IncaText,
} from "./types.mts";

/** The builders a clone uses for each kind of handle, and the lookups every handle uses. */
const builders: HandleBuilders = {
  element: createIncaElement,
  text: createIncaText,
  comment: createIncaComment,
  createText: createTextNode,
  createComment: createCommentNode,
  fragment: createIncaFragment,
  document: documentHandle,
  documentCopy: (id) => createIncaDocument(id, true),
  node: handleOf,
};

installInterfaces(builders);

/** The passive events of `document`, shared by its handles. */
const WHEEL_EVENTS: readonly string[] = ["wheel"];

/** The `content` of an HTML `template`, an own accessor of its handle. */
const CONTENT: PropertyDescriptor = {
  get(this: unknown) {
    return existingContent(receiverNode(this, HTML_ELEMENT_RULE));
  },
  enumerable: true,
  configurable: true,
};

/** The `value` of an HTML `input`, an own accessor of its handle. */
const VALUE: PropertyDescriptor = {
  get(this: unknown) {
    const element = receiverNode(this, HTML_ELEMENT_RULE);
    return isDestroyed(element) ? "" : inputValue(element.__node_id);
  },
  set(this: unknown, value: unknown) {
    const element = receiverNode(this, HTML_ELEMENT_RULE);
    if (isDestroyed(element)) return;
    // oxlint-disable-next-line typescript/no-base-to-string
    setInputValue(element.__node_id, value === null ? "" : String(value));
  },
  enumerable: true,
  configurable: true,
};

/**
 * Builds the handle of a native element node: parentless and childless, on the prototype of its
 * interface with its node id and state. The handle keeps the name, so the name members read after
 * the host frees the node.
 * @param id - the native node's id
 * @param name - the element name, an HTML `div` when omitted. An HTML `input` has `value` and an
 *   HTML `template` has `content`.
 * @param passiveByDefault - event names whose listeners are passive unless their options say
 *   `passive: false`
 * @returns the new handle
 */
export function createIncaElement(
  id: NodeId,
  name?: string,
  passiveByDefault?: readonly string[],
): IncaHTMLElement;
export function createIncaElement(
  id: NodeId,
  name?: ElementName,
  passiveByDefault?: readonly string[],
): IncaElement;
export function createIncaElement(
  id: NodeId,
  name: ElementName = "div",
  passiveByDefault: readonly string[] = NO_EVENTS,
): IncaElement {
  const localName = typeof name === "string" ? name : name[0];
  const namespaceURI = typeof name === "string" ? HTML_NAMESPACE : name[1];
  const element = withState(
    Object.create(prototypeOf("element", name)) as IncaElement,
    id,
    "element",
    passiveByDefault,
    name,
  );
  const html = namespaceURI === HTML_NAMESPACE;
  if (html && localName === "template") Object.defineProperty(element, "content", CONTENT);
  if (html && localName === "input") Object.defineProperty(element, "value", VALUE);
  registerNode(element);
  if (html && localName === "template") attachContent(element, createIncaFragment);
  return element;
}

/**
 * Allocates a new element node for `tag`.
 * @param tag - the node's element kind
 * @returns the new, parentless, childless element
 */
export function createElementNode(tag: TagName): IncaHTMLElement {
  const id = createNode(tag);
  return createIncaElement(id, tag);
}

// The host has no comment node. A hidden, childless container holds the data of a comment.
/**
 * Creates a comment, such as a placeholder for conditional content.
 * @param text - the data of the comment
 * @returns the new comment
 */
export function createCommentNode(text = ""): IncaComment {
  const id = createNode("#comment");
  setStyle(id, "display", "none");
  setAttribute(id, "value", text);
  return createIncaComment(id);
}

/**
 * Builds the handle of a comment: an object of `Comment.prototype` with its node id and state.
 * @param id - the native node's id
 * @returns the new, parentless handle
 */
export function createIncaComment(id: NodeId): IncaComment {
  const comment = withState(
    Object.create(prototypeOf("comment")) as IncaComment,
    id,
    "comment",
    NO_EVENTS,
  );
  registerNode(comment);
  return comment;
}

/**
 * Creates a text leaf.
 * @param text - the initial text content
 * @returns the new text leaf
 */
export function createTextNode(text: string): IncaText {
  const id = createNode("#text");
  setAttribute(id, "value", text);
  return createIncaText(id, text);
}

/**
 * Builds the handle of an existing text node.
 * @param id - the native node's id
 * @param data - the data the node holds
 * @returns the new, parentless handle
 */
export function adoptTextNode(id: NodeId, data: string): IncaText {
  return createIncaText(id, data);
}

/**
 * Replaces all of `el`'s children with at most a single text child holding
 * `text`. The old children keep their listeners. An empty string leaves `el` childless.
 * @param el - the element or fragment whose children are replaced
 * @param text - the new text content
 */
export function setElementText(el: IncaParent, text: string): void {
  replaceWithText(el, text, builders);
}

/**
 * Returns the handle of the host's mount container, which stands between the app's elements and
 * `document` in an event path. A live handle is reused, so a mount and an event see one object.
 * A `wheel` listener on it is passive unless its options say `passive: false`.
 * @returns the mount container's handle
 */
export function rootElement(): IncaHTMLElement {
  const id = rootNodeId();
  const live = elementOf(id);
  if (live) return live as IncaHTMLElement;
  const root = createIncaElement(id, "div", WHEEL_EVENTS);
  registerRoot(root);
  return root;
}

/**
 * Builds the handle of a fragment.
 * @param id - the native node's id
 * @returns the new, empty handle
 */
export function createIncaFragment(id: NodeId): IncaFragment {
  const fragment = withState(
    Object.create(prototypeOf("fragment")) as IncaFragment,
    id,
    "fragment",
    NO_EVENTS,
  );
  registerNode(fragment);
  return fragment;
}

/** The handle of the document, built on first use and kept for the life of the realm. */
let theDocument: IncaDocument | undefined;

/**
 * Returns the handle of the document.
 * @returns the handle, which names the same object on every call
 */
export function documentHandle(): IncaDocument {
  return (theDocument ??= createIncaDocument(documentNodeId(), false));
}

/** The handle of the window, built on first use and kept for the life of the realm. */
let theWindow: IncaWindow | undefined;

/**
 * Returns the handle of the window.
 * @returns the handle, which names the same object on every call
 */
export function windowHandle(): IncaWindow {
  if (theWindow === undefined) {
    const handle = createEventTarget(windowTargetId, WHEEL_EVENTS);
    defineMembers(handle, windowHandlerMembers());
    Object.defineProperty(handle, Symbol.toStringTag, { value: "Window" });
    theWindow = handle as IncaWindow;
  }
  return theWindow;
}

/**
 * Looks up the handle of a node by id, the document handle included. A collected handle of the
 * root container is rebuilt.
 * @param id - the native node's id
 * @returns the live handle, or `undefined` when none is live
 */
export function handleOf(id: NodeId): IncaNode | undefined {
  const live = nodeOf(id);
  if (live) return live;
  if (id === rootNodeId()) return rootElement();
  return id === documentNodeId() ? documentHandle() : undefined;
}

/**
 * Builds the handle of a document node: an object of `HTMLDocument.prototype` with its node id and
 * state.
 * @param id - the native node's id
 * @param copy - whether the node is a copy of the document, which the focus of the host never
 *   reaches
 * @returns the new handle
 */
function createIncaDocument(id: NodeId, copy: boolean): IncaDocument {
  const handle = withState(
    Object.create(prototypeOf("document")) as IncaDocument,
    id,
    "document",
    WHEEL_EVENTS,
  );
  stateOf(handle)!.copy = copy;
  registerNode(handle);
  return handle;
}
