// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  blur,
  click,
  focus,
  hasPointerCapture,
  releasePointerCapture,
  setPointerCapture,
} from "../events.mts";
import { createEventTarget } from "../eventTarget.mts";
import {
  appendChild,
  checkSelector,
  closestNode,
  createElement as createHostElement,
  createNode,
  documentNodeId,
  focusedNode,
  getAttribute,
  inputValue,
  matchesNode,
  queryNode,
  queryNodes,
  removeChild,
  rootNodeId,
  scrollIntoViewNode,
  scrollOf,
  setAttribute,
  setInputValue,
  setStyle,
  textContent,
} from "../tree.mts";
import { attributeMembers, attributeText, requireArguments } from "./attributes.mts";
import { classMembers } from "./classList.mts";
import { nodeListOf } from "./collections.mts";
import { elementTreeMembers } from "./elementTree.mts";
import { withEventHandlers } from "./eventHandlers.mts";
import { layoutBox, offsetOf, offsetParentOf, rectOf, tabIndexOf } from "./geometry.mts";
import { htmlMembers } from "./html.mts";
import {
  elementOf,
  existingContent,
  isDestroyed,
  isRemoved,
  nodeOf,
  registerNode,
  withNodeId,
} from "./lifecycle.mts";
import { nodeMutationMembers } from "./nodeMutation.mts";
import { nodeRelationMembers } from "./nodeRelations.mts";
import { nodeTreeMembers } from "./nodeTree.mts";
import { scrollIntoViewArgs, scrollPosition } from "./scroll.mts";
import { keep, unlink } from "./shadowTree.mts";
import { attachContent, isTemplateTag } from "./templateContent.mts";
import { characterDataMembers, createIncaText, nullableText } from "./text.mts";
import type { IncaDocument, NodeId, TagName } from "../types.mts";
import type { AttributeMembers } from "./attributes.mts";
import type { ClassMembers } from "./classList.mts";
import type { ElementTreeMembers } from "./elementTree.mts";
import type { HandleBuilders } from "./nodeMutation.mts";
import type { ScrollArgs, ScrollIntoViewOptions } from "./scroll.mts";
import type {
  EventHandlers,
  HtmlMembers,
  IncaElement,
  IncaFragment,
  IncaNode,
  IncaParent,
  IncaText,
  NodeMembers,
  TextContentMember,
} from "./types.mts";

/**
 * Checks the argument count of a selector method and converts the selector to text.
 * @param args - the arguments the call received
 * @param method - the method name for the message
 * @returns the selector text
 */
function selectorText(args: readonly unknown[], method: string): string {
  requireArguments(args, 1, method);
  return attributeText(args[0]);
}

/** The members `querySelector()` and `querySelectorAll()` of an element and of the document. */
type QueryMembers = Pick<IncaElement, "querySelector" | "querySelectorAll">;

/**
 * Finds the element handle of an element node.
 * @param id - the native node's id
 * @returns the handle
 */
function elementHandle(id: NodeId): IncaElement {
  return handleOf(id) as IncaElement;
}

/**
 * Builds the query members of a node that holds elements below it.
 * @param node - the element or the document the members belong to
 * @returns the members
 */
function queryMembers(node: IncaElement | IncaDocument): QueryMembers {
  const id = node.__node_id;
  return {
    querySelector: (...args: [selectors: string]) => {
      const text = selectorText(args, "querySelector");
      if (isDestroyed(node)) {
        checkSelector(text);
        return null;
      }
      const found = queryNode(id, text);
      return found === null ? null : elementHandle(found);
    },
    querySelectorAll: (...args: [selectors: string]) => {
      const text = selectorText(args, "querySelectorAll");
      if (isDestroyed(node)) {
        checkSelector(text);
        return nodeListOf(() => []);
      }
      const found = queryNodes(id, text).map(elementHandle);
      return nodeListOf(() => found);
    },
  };
}

/** The builders a clone uses for each kind of handle, and the lookups every handle uses. */
const builders: HandleBuilders = {
  element: createIncaElement,
  text: (id, data) => createIncaText(id, data, builders),
  fragment: createIncaFragment,
  document: documentHandle,
  node: handleOf,
};

/**
 * Builds the {@link IncaElement} handle for a native node: parentless,
 * childless, with `id`, `focus()`, `blur()`, `click()`, the pointer capture
 * methods, the listener methods of an event target, `tabIndex`,
 * the `value` of an `input`, the layout members, the node members and the event
 * handler properties, all driven through the host.
 * @param id - the native node's id
 * @param kind - whether the node is an element or a comment stand-in
 * @param tag - the node's element kind, `div` when omitted; a `button` or `input` defaults to tab
 *   index `0`, and an `input` has `value`
 * @param passiveByDefault - event names whose listeners are passive unless their options say
 *   `passive: false`
 * @returns the new handle
 */
export function createIncaElement(
  id: NodeId,
  kind: IncaElement["kind"],
  tag: TagName = "div",
  passiveByDefault: readonly string[] = [],
): IncaElement {
  const target = createEventTarget(() => id, passiveByDefault);
  const scroll = () => (isRemoved(element) ? null : scrollOf(id));
  const fields: Omit<
    IncaElement,
    | "__node_id"
    | keyof EventHandlers
    | keyof AttributeMembers
    | keyof ClassMembers
    | keyof ElementTreeMembers
    | keyof HtmlMembers
    | keyof NodeMembers
    | keyof TextContentMember
    | keyof QueryMembers
  > = {
    get id() {
      return isDestroyed(element) ? "" : String(getAttribute(id, "id") ?? "");
    },
    set id(value: string) {
      if (!isDestroyed(element)) setAttribute(id, "id", String(value));
    },
    kind,
    tagName: tag.replace(/[a-z]/g, (letter) => letter.toUpperCase()),
    parent: null,
    __children: [],
    focus: (options?: { preventScroll?: boolean }) => {
      if (!isRemoved(element)) focus(id, Boolean(options?.preventScroll));
    },
    blur: () => blur(id),
    click: () => {
      if (!isRemoved(element)) click(id);
    },
    setPointerCapture: (pointerId: number) => setPointerCapture(id, pointerId),
    releasePointerCapture: (pointerId: number) => releasePointerCapture(id, pointerId),
    hasPointerCapture: (pointerId: number) => hasPointerCapture(id, pointerId),
    addEventListener: (...args: Parameters<typeof target.addEventListener>) => {
      if (!isRemoved(element)) target.addEventListener(...args);
    },
    removeEventListener: (...args: Parameters<typeof target.removeEventListener>) => {
      if (!isRemoved(element)) target.removeEventListener(...args);
    },
    dispatchEvent: (event) => isDestroyed(element) || target.dispatchEvent(event),
    get tabIndex() {
      return tabIndexOf(isDestroyed(element) ? null : getAttribute(id, "tabindex"), tag);
    },
    set tabIndex(value: number) {
      if (!isDestroyed(element)) setAttribute(id, "tabindex", value | 0);
    },
    getBoundingClientRect: () => rectOf(layoutBox(element)),
    get offsetWidth() {
      return Math.round(layoutBox(element)?.[2] ?? 0);
    },
    get offsetHeight() {
      return Math.round(layoutBox(element)?.[3] ?? 0);
    },
    get offsetLeft() {
      return offsetOf(element).left;
    },
    get offsetTop() {
      return offsetOf(element).top;
    },
    get offsetParent() {
      return offsetParentOf(element);
    },
    get clientWidth() {
      const box = layoutBox(element);
      return box ? Math.max(0, Math.round(box[2] - 2 * box[4])) : 0;
    },
    get clientHeight() {
      const box = layoutBox(element);
      return box ? Math.max(0, Math.round(box[3] - 2 * box[4])) : 0;
    },
    get clientLeft() {
      return Math.round(layoutBox(element)?.[4] ?? 0);
    },
    get clientTop() {
      return Math.round(layoutBox(element)?.[4] ?? 0);
    },
    get scrollLeft() {
      return scroll()?.[0] ?? 0;
    },
    set scrollLeft(value: number) {
      scrollPosition(element, [value], false);
    },
    get scrollTop() {
      return scroll()?.[1] ?? 0;
    },
    set scrollTop(value: number) {
      scrollPosition(element, [undefined, value], false);
    },
    get scrollWidth() {
      return element.clientWidth + Math.round(scroll()?.[2] ?? 0);
    },
    get scrollHeight() {
      return element.clientHeight + Math.round(scroll()?.[3] ?? 0);
    },
    scrollTo: (...args: ScrollArgs) => scrollPosition(element, args, false),
    scrollBy: (...args: ScrollArgs) => scrollPosition(element, args, true),
    scrollIntoView: (arg?: boolean | ScrollIntoViewOptions) => {
      if (isRemoved(element)) return;
      const { block, inline } = scrollIntoViewArgs(arg);
      scrollIntoViewNode(id, block, inline);
    },
    matches: (...args: [selectors: string]) => {
      const text = selectorText(args, "matches");
      if (isDestroyed(element)) {
        checkSelector(text);
        return false;
      }
      return matchesNode(id, text);
    },
    closest: (...args: [selectors: string]) => {
      const text = selectorText(args, "closest");
      if (isDestroyed(element)) {
        checkSelector(text);
        return null;
      }
      const found = closestNode(id, text);
      return found === null ? null : elementHandle(found);
    },
  };
  if (isTemplateTag(tag)) {
    Object.defineProperty(fields, "content", {
      get: () => existingContent(element),
      enumerable: true,
      configurable: true,
    });
  }
  if (tag === "input") {
    Object.defineProperty(fields, "value", {
      get: () => (isDestroyed(element) ? "" : inputValue(id)),
      set: (value: unknown) => {
        if (isDestroyed(element)) return;
        // oxlint-disable-next-line typescript/no-base-to-string
        setInputValue(id, value === null ? "" : String(value));
      },
      enumerable: true,
      configurable: true,
    });
  }
  const element: IncaElement = withEventHandlers(withNodeId(fields as IncaElement, id), () =>
    isRemoved(element),
  );
  Object.defineProperties(element, Object.getOwnPropertyDescriptors(attributeMembers(id, element)));
  Object.defineProperties(element, Object.getOwnPropertyDescriptors(elementTreeMembers(element)));
  Object.defineProperties(
    element,
    Object.getOwnPropertyDescriptors(nodeTreeMembers(element, builders)),
  );
  Object.defineProperties(
    element,
    Object.getOwnPropertyDescriptors(nodeMutationMembers(element, builders)),
  );
  Object.defineProperties(
    element,
    Object.getOwnPropertyDescriptors(nodeRelationMembers(element, builders)),
  );
  Object.defineProperties(element, Object.getOwnPropertyDescriptors(queryMembers(element)));
  Object.defineProperties(
    element,
    Object.getOwnPropertyDescriptors(
      kind === "comment" ? characterDataMembers(element) : elementTextContent(element),
    ),
  );
  if (kind !== "comment") {
    Object.defineProperties(element, Object.getOwnPropertyDescriptors(classMembers(id, element)));
    Object.defineProperties(
      element,
      Object.getOwnPropertyDescriptors(htmlMembers(element, builders)),
    );
  }
  registerNode(element);
  if (isTemplateTag(tag)) attachContent(element, createIncaFragment);
  return element;
}

/**
 * Builds the `textContent` of an element.
 * @param element - the handle the member belongs to
 * @returns the member
 */
function elementTextContent(element: IncaElement): TextContentMember {
  return {
    get textContent() {
      return isDestroyed(element) ? "" : textContent(element.__node_id);
    },
    set textContent(value: string) {
      if (isDestroyed(element)) return;
      setElementText(element, nullableText(value));
    },
  };
}

/**
 * Allocates a new element node for `tag`.
 * @param tag - the node's element kind
 * @returns the new, parentless, childless element
 */
export function createElementNode(tag: TagName): IncaElement {
  const id = createNode(tag);
  return createIncaElement(id, "element", tag);
}

// inca has no comment concept. A hidden, childless container stands in for one.
/**
 * Creates the hidden container that stands in for a comment node, such as a
 * placeholder for conditional content.
 * @param text - the data of the comment
 * @returns the new hidden element
 */
export function createCommentNode(text = ""): IncaElement {
  const id = createNode("#comment");
  setStyle(id, "display", "none");
  setAttribute(id, "value", text);
  return createIncaElement(id, "comment");
}

/**
 * Creates a text leaf.
 * @param text - the initial text content
 * @returns the new text leaf
 */
export function createTextNode(text: string): IncaText {
  const id = createNode("#text");
  setAttribute(id, "value", text);
  return createIncaText(id, text, builders);
}

/**
 * Builds the handle of an existing text node.
 * @param id - the native node's id
 * @param data - the data the node holds
 * @returns the new, parentless handle
 */
export function adoptTextNode(id: NodeId, data: string): IncaText {
  return createIncaText(id, data, builders);
}

/**
 * Replaces all of `el`'s children with at most a single text child holding
 * `text`. The old children keep their listeners. An empty string leaves `el` childless.
 * @param el - the element or fragment whose children are replaced
 * @param text - the new text content
 */
export function setElementText(el: IncaParent, text: string): void {
  for (const child of el.__children.slice()) {
    removeChild(el.__node_id, child.__node_id);
    unlink(child);
    keep(child);
  }
  if (!text) return;

  const textNode = createTextNode(text);
  appendChild(el.__node_id, textNode.__node_id);
  textNode.parent = el;
  el.__children.push(textNode);
}

/**
 * Returns the handle of the host's mount container, which stands between the app's elements and
 * `document` in an event path. A live handle is reused, so a mount and an event see one object.
 * A `wheel` listener on it is passive unless its options say `passive: false`.
 * @returns the mount container's handle
 */
export function rootElement(): IncaElement {
  const id = rootNodeId();
  return elementOf(id) ?? createIncaElement(id, "element", undefined, ["wheel"]);
}

/**
 * Builds the handle of a fragment.
 * @param id - the native node's id
 * @returns the new, empty handle
 */
export function createIncaFragment(id: NodeId): IncaFragment {
  const fields: Omit<IncaFragment, "__node_id" | keyof NodeMembers | keyof TextContentMember> = {
    kind: "fragment",
    parent: null,
    __children: [],
  };
  const fragment = withNodeId(fields as IncaFragment, id);
  const textMember: TextContentMember = {
    get textContent() {
      return isDestroyed(fragment) ? "" : textContent(id);
    },
    set textContent(value: string) {
      if (!isDestroyed(fragment)) setElementText(fragment, nullableText(value));
    },
  };
  for (const members of [
    nodeTreeMembers(fragment, builders),
    nodeMutationMembers(fragment, builders),
    nodeRelationMembers(fragment, builders),
    textMember,
  ]) {
    Object.defineProperties(fragment, Object.getOwnPropertyDescriptors(members));
  }
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
  return (theDocument ??= createIncaDocument(documentNodeId()));
}

/**
 * Looks up the handle of a node by id, the document handle included.
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
 * Holds a node the script created until its handle is collected.
 * @param handle - the new, parentless handle
 * @returns `handle`
 */
function held<T extends IncaNode>(handle: T): T {
  keep(handle);
  return handle;
}

/**
 * Builds the handle of the document node: the listener methods and handler properties of an event
 * target, the node members without `cloneNode()`, the creation methods and the query methods.
 * @param id - the native node's id
 * @returns the new handle
 */
function createIncaDocument(id: NodeId): IncaDocument {
  const target = createEventTarget(id, ["wheel"]);
  const fields: Omit<
    IncaDocument,
    "__node_id" | keyof EventHandlers<IncaDocument> | keyof NodeMembers | keyof QueryMembers
  > = {
    ...target,
    kind: "document",
    parent: null,
    __children: [],
    get textContent() {
      return null;
    },
    set textContent(_value: string | null) {},
    get activeElement() {
      const focused = focusedNode();
      return focused === null ? null : (elementOf(focused) ?? null);
    },
    createDocumentFragment: () => held(createIncaFragment(createNode("#fragment"))),
    createElement: (...args: [localName: string]) => {
      requireArguments(args, 1, "createElement");
      const [elementId, tag] = createHostElement(attributeText(args[0]));
      return held(createIncaElement(elementId, "element", tag));
    },
    createTextNode: (...args: [data: string]) => {
      requireArguments(args, 1, "createTextNode");
      return held(createTextNode(attributeText(args[0])));
    },
    createComment: (...args: [data: string]) => {
      requireArguments(args, 1, "createComment");
      return held(createCommentNode(attributeText(args[0])));
    },
  };
  const handle = withEventHandlers(withNodeId(fields as IncaDocument, id), () => false);
  // The host has one document, so the document has no `cloneNode()`.
  const { cloneNode: _cloneNode, ...mutation } = nodeMutationMembers(handle, builders);
  for (const members of [
    nodeTreeMembers(handle, builders),
    mutation,
    nodeRelationMembers(handle, builders),
    queryMembers(handle),
  ]) {
    Object.defineProperties(handle, Object.getOwnPropertyDescriptors(members));
  }
  Object.setPrototypeOf(handle, Document.prototype);
  registerNode(handle);
  return handle;
}
