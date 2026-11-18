// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createEventTarget } from "../eventTarget.mts";
import {
  createNode,
  documentNodeId,
  inputValue,
  nodeHandle,
  rootNodeId,
  setCharacterData,
  setInputValue,
  textContent,
  windowTargetId,
} from "../ffi/index.mts";
import { installInterfaces, defineMembers, windowHandlerMembers } from "./interfaces.mts";
import {
  elementOf,
  entered,
  existingContent,
  hold,
  isDestroyed,
  nodeOf,
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
import type { Document, Window, NodeId, TagName } from "../types.mts";
import type { HandleNode } from "./handleNode.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { ParentNode } from "./nodeTypes.mts";
import type { Comment, Element, DocumentFragment, HTMLElement, Text } from "./types.mts";

/** The lookups the members use. */
const builders: HandleBuilders = {
  document: documentHandle,
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
 * Builds the handle of an existing host node: a new instance of the host class on the prototype of
 * its kind, with its state. An HTML `input` has `value`, an HTML `template` has `content`, and a
 * text node has `text`. The handle is recorded weakly.
 * @param id - the native node's id
 * @param passiveByDefault - event names whose listeners are passive unless their options say
 *   `passive: false`. A document passes `wheel` when this is omitted.
 * @param data - the data of a text node, read from the host when omitted
 * @returns the handle and whether its node has a parent, or `undefined` for an id the host no
 *   longer has
 */
function buildHandle(
  id: NodeId,
  passiveByDefault?: readonly string[],
  data?: string,
): { handle: HandleNode; parented: boolean } | undefined {
  const built = nodeHandle(id);
  if (built === null) return undefined;
  const [instance, parented, special] = built;
  const handle = instance as HandleNode;
  withState(handle, passiveByDefault ?? NO_EVENTS);
  const state = stateOf(handle)!;
  if (state.kind === "document") {
    state.passive = passiveByDefault ?? WHEEL_EVENTS;
    state.copy = id !== documentNodeId();
  }
  if (special === "template") Object.defineProperty(handle, "content", CONTENT);
  if (special === "input") Object.defineProperty(handle, "value", VALUE);
  if (state.kind === "text") (handle as Text).text = data ?? textContent(id);
  registerNode(handle);
  if (special === "template")
    attachContent(handle as Element, (content) => adoptHandle(content) as DocumentFragment);
  return { handle, parented };
}

/**
 * Builds the handle of a node the host just created or copied, and records it weakly.
 * @param id - the native node's id
 * @param passiveByDefault - event names whose listeners are passive unless their options say
 *   `passive: false`
 * @param data - the data of a text node, read from the host when omitted
 * @returns the new handle
 */
export function adoptHandle(
  id: NodeId,
  passiveByDefault?: readonly string[],
  data?: string,
): HandleNode {
  return buildHandle(id, passiveByDefault, data)!.handle;
}

/**
 * Allocates a new element node for `tag`.
 * @param tag - the node's element kind
 * @returns the new, parentless, childless element
 */
export function createElementNode(tag: TagName): HTMLElement {
  return adoptHandle(createNode(tag)) as HTMLElement;
}

/**
 * Creates a comment, such as a placeholder for conditional content.
 * @param text - the data of the comment
 * @returns the new comment
 */
export function createCommentNode(text = ""): Comment {
  const id = createNode("#comment");
  setCharacterData(id, text);
  return adoptHandle(id) as Comment;
}

/**
 * Creates a text leaf.
 * @param text - the initial text content
 * @returns the new text leaf
 */
export function createTextNode(text: string): Text {
  const id = createNode("#text");
  setCharacterData(id, text);
  return adoptHandle(id, NO_EVENTS, text) as Text;
}

/**
 * Replaces all of `el`'s children with at most a single text child holding
 * `text`. The old children keep their listeners. An empty string leaves `el` childless.
 * @param el - the element or fragment whose children are replaced
 * @param text - the new text content
 */
export function setElementText(el: ParentNode, text: string): void {
  replaceWithText(el, text, builders);
}

/**
 * Returns the handle of the host's mount container, which stands between the app's elements and
 * `document` in an event path. A live handle is reused, so a mount and an event see one object.
 * A `wheel` listener on it is passive unless its options say `passive: false`.
 * @returns the mount container's handle
 */
export function rootElement(): HTMLElement {
  const id = rootNodeId();
  const live = elementOf(id);
  if (live) return live as HTMLElement;
  const root = adoptHandle(id, WHEEL_EVENTS) as HTMLElement;
  registerRoot(root);
  return root;
}

/** The handle of the document, built on first use and kept for the life of the realm. */
let theDocument: Document | undefined;

/**
 * Returns the handle of the document.
 * @returns the handle, which names the same object on every call
 */
export function documentHandle(): Document {
  return (theDocument ??= adoptHandle(documentNodeId()) as Document);
}

/** The handle of the window, built on first use and kept for the life of the realm. */
let theWindow: Window | undefined;

/**
 * Returns the handle of the window.
 * @returns the handle, which names the same object on every call
 */
export function windowHandle(): Window {
  if (theWindow === undefined) {
    const handle = createEventTarget(windowTargetId, WHEEL_EVENTS);
    defineMembers(handle, windowHandlerMembers());
    Object.defineProperty(handle, Symbol.toStringTag, { value: "Window" });
    theWindow = handle as Window;
  }
  return theWindow;
}

/**
 * Looks up the handle of a node by id, the document handle included, and builds it when none is
 * live. A built handle of a node with a parent is strong, and a built handle of a node with none
 * is weak and held. A collected handle of the root container is rebuilt.
 * @param id - the native node's id
 * @param data - the data of a text node, read from the host when omitted
 * @returns the handle, or `undefined` for an id the host no longer has
 */
export function handleOf(id: NodeId, data?: string): HandleNode | undefined {
  const live = nodeOf(id);
  if (live) return live;
  if (id === rootNodeId()) return rootElement();
  if (id === documentNodeId()) return documentHandle();
  const built = buildHandle(id, undefined, data);
  if (built === undefined) return undefined;
  if (built.parented) entered(built.handle);
  else hold(built.handle);
  return built.handle;
}
