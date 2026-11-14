// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createEventTarget } from "../eventTarget.mts";
import {
  blur,
  click,
  focus,
  hasPointerCapture,
  releasePointerCapture,
  setPointerCapture,
  HTML_NAMESPACE,
  appendChild,
  childNodeIds,
  createElement as createHostElement,
  createElementNS as createHostElementNS,
  createNode,
  documentNodeId,
  focusedNode,
  getAttribute,
  inputValue,
  linkedNode,
  nameOf,
  removeChild,
  rootNodeId,
  setAttribute,
  setInputValue,
  setStyle,
  textContent,
} from "../ffi/index.mts";
import { attributeText, nullableText, requireArguments } from "./arguments.mts";
import { attributeMembers } from "./attributes.mts";
import { classMembers } from "./classList.mts";
import { datasetMembers } from "./dataset.mts";
import { elementTreeMembers } from "./elementTree.mts";
import { withEventHandlers } from "./eventHandlers.mts";
import { layoutMembers, tabIndexOf } from "./geometry.mts";
import { htmlMembers } from "./html.mts";
import { styleMembers } from "./inlineStyle.mts";
import {
  elementOf,
  entered,
  existingContent,
  hold,
  isDestroyed,
  isRemoved,
  left,
  nodeOf,
  registerNode,
  registerRoot,
  withNodeId,
} from "./lifecycle.mts";
import { nodeMutationMembers } from "./nodeMutation.mts";
import { nodeRelationMembers } from "./nodeRelations.mts";
import { nodeTreeMembers } from "./nodeTree.mts";
import { scrollMembers } from "./scroll.mts";
import { queryMembers, selectorMembers } from "./selectors.mts";
import { attachContent } from "./templateContent.mts";
import { characterDataMembers, createIncaText } from "./text.mts";
import type { ElementName } from "../ffi/index.mts";
import type { IncaDocument, NodeId, TagName } from "../types.mts";
import type { AttributeMembers } from "./attributes.mts";
import type { ClassMembers } from "./classList.mts";
import type { DatasetMembers } from "./dataset.mts";
import type { ElementTreeMembers } from "./elementTree.mts";
import type { LayoutMembers } from "./geometry.mts";
import type { StyleMembers } from "./inlineStyle.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { ScrollMembers } from "./scroll.mts";
import type { QueryMembers, SelectorMembers } from "./selectors.mts";
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

/** The builders a clone uses for each kind of handle, and the lookups every handle uses. */
const builders: HandleBuilders = {
  element: createIncaElement,
  text: (id, data) => createIncaText(id, data, builders),
  fragment: createIncaFragment,
  document: documentHandle,
  documentCopy: (id) => createIncaDocument(id, true),
  node: handleOf,
};

/**
 * Builds the {@link IncaElement} handle for a native node: parentless,
 * childless, with `id`, `focus()`, `blur()`, `click()`, the pointer capture
 * methods, the listener methods of an event target, `tabIndex`,
 * the `value` of an `input`, the `style`, the `dataset`, the `classList`, the layout members,
 * the node members and the event handler properties, all driven through the host.
 * @param id - the native node's id
 * @param kind - whether the node is an element or a comment stand-in
 * @param name - the element name, an HTML `div` when omitted. An HTML `button` or `input`
 *   defaults to tab index `0`, an HTML `input` has `value` and an HTML `template` has `content`.
 *   The handle keeps the name, so the name members read after the host frees the node.
 * @param passiveByDefault - event names whose listeners are passive unless their options say
 *   `passive: false`
 * @returns the new handle
 */
export function createIncaElement(
  id: NodeId,
  kind: IncaElement["kind"],
  name: ElementName = "div",
  passiveByDefault: readonly string[] = [],
): IncaElement {
  const [localName, namespaceURI, prefix, tagName] =
    typeof name === "string"
      ? [name, HTML_NAMESPACE, null, name.replace(/[a-z]/g, (letter) => letter.toUpperCase())]
      : name;
  const html = namespaceURI === HTML_NAMESPACE;
  const target = createEventTarget(() => id, passiveByDefault);
  const fields: Omit<
    IncaElement,
    | "__node_id"
    | "tagName"
    | "localName"
    | "namespaceURI"
    | "prefix"
    | keyof EventHandlers
    | keyof AttributeMembers
    | keyof ClassMembers
    | keyof DatasetMembers
    | keyof ElementTreeMembers
    | keyof HtmlMembers
    | keyof NodeMembers
    | keyof TextContentMember
    | keyof LayoutMembers
    | keyof QueryMembers
    | keyof ScrollMembers
    | keyof SelectorMembers
    | keyof StyleMembers
  > = {
    get id() {
      return isDestroyed(element) ? "" : String(getAttribute(id, "id") ?? "");
    },
    set id(value: string) {
      if (!isDestroyed(element)) setAttribute(id, "id", String(value));
    },
    kind,
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
      return tabIndexOf(
        isDestroyed(element) ? null : getAttribute(id, "tabindex"),
        html ? localName : "",
      );
    },
    set tabIndex(value: number) {
      if (!isDestroyed(element)) setAttribute(id, "tabindex", value | 0);
    },
  };
  for (const [member, value] of Object.entries({ tagName, localName, namespaceURI, prefix })) {
    Object.defineProperty(fields, member, { value, enumerable: true });
  }
  const template = html && localName === "template";
  if (template) {
    Object.defineProperty(fields, "content", {
      get: () => existingContent(element),
      enumerable: true,
      configurable: true,
    });
  }
  if (html && localName === "input") {
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
  Object.defineProperties(
    element,
    Object.getOwnPropertyDescriptors(elementTreeMembers(element, builders)),
  );
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
  for (const members of [
    layoutMembers(element, builders),
    scrollMembers(element),
    selectorMembers(element, builders),
    queryMembers(element, builders),
  ]) {
    Object.defineProperties(element, Object.getOwnPropertyDescriptors(members));
  }
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
    Object.defineProperties(element, Object.getOwnPropertyDescriptors(styleMembers(id)));
    Object.defineProperties(element, Object.getOwnPropertyDescriptors(datasetMembers(id)));
  }
  registerNode(element);
  if (template) attachContent(element, createIncaFragment);
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
  for (const childId of childNodeIds(el.__node_id)) {
    removeChild(el.__node_id, childId);
    const child = nodeOf(childId);
    if (child) left(child);
  }
  if (!text) return;

  const textNode = createTextNode(text);
  appendChild(el.__node_id, textNode.__node_id);
  entered(textNode);
}

/**
 * Returns the handle of the host's mount container, which stands between the app's elements and
 * `document` in an event path. A live handle is reused, so a mount and an event see one object.
 * A `wheel` listener on it is passive unless its options say `passive: false`.
 * @returns the mount container's handle
 */
export function rootElement(): IncaElement {
  const id = rootNodeId();
  const live = elementOf(id);
  if (live) return live;
  const root = createIncaElement(id, "element", "div", ["wheel"]);
  registerRoot(root);
  return root;
}

/**
 * Builds the handle of a fragment.
 * @param id - the native node's id
 * @returns the new, empty handle
 */
export function createIncaFragment(id: NodeId): IncaFragment {
  const fields: Omit<IncaFragment, "__node_id" | keyof NodeMembers | keyof TextContentMember> = {
    kind: "fragment",
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
  return (theDocument ??= createIncaDocument(documentNodeId(), false));
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
 * Holds a node the script created until its handle is collected.
 * @param handle - the new, parentless handle
 * @returns `handle`
 */
function held<T extends IncaNode>(handle: T): T {
  hold(handle);
  return handle;
}

/**
 * Builds the handle of a document node: the listener methods and handler properties of an event
 * target, the node members, the creation methods and the query methods.
 * @param id - the native node's id
 * @param copy - whether the node is a copy of the document, which the focus of the host never
 *   reaches
 * @returns the new handle
 */
function createIncaDocument(id: NodeId, copy: boolean): IncaDocument {
  const target = createEventTarget(id, ["wheel"]);
  const fields: Omit<
    IncaDocument,
    "__node_id" | keyof EventHandlers<IncaDocument> | keyof NodeMembers | keyof QueryMembers
  > = {
    ...target,
    kind: "document",
    get textContent() {
      return null;
    },
    set textContent(_value: string | null) {},
    get activeElement() {
      const focused = copy ? null : focusedNode();
      if (focused !== null) return elementOf(focused) ?? null;
      const first = linkedNode(id, "first", true);
      const element = first === null ? undefined : builders.node(first);
      return element?.kind === "element" ? element : null;
    },
    createDocumentFragment: () => held(createIncaFragment(createNode("#fragment"))),
    createElement: (...args: [localName: string]) => {
      requireArguments(args, 1, "createElement");
      const entry = createHostElement(attributeText(args[0]));
      return held(createIncaElement(entry[0], "element", nameOf(entry)));
    },
    createElementNS: (...args: [namespace: string | null, qualifiedName: string]) => {
      requireArguments(args, 2, "createElementNS");
      const namespace = args[0] === null || args[0] === undefined ? null : attributeText(args[0]);
      const entry = createHostElementNS(namespace || null, attributeText(args[1]));
      return held(createIncaElement(entry[0], "element", nameOf(entry)));
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
  for (const members of [
    nodeTreeMembers(handle, builders),
    nodeMutationMembers(handle, builders),
    nodeRelationMembers(handle, builders),
    queryMembers(handle, builders),
  ]) {
    Object.defineProperties(handle, Object.getOwnPropertyDescriptors(members));
  }
  Object.setPrototypeOf(handle, Document.prototype);
  registerNode(handle);
  return handle;
}
