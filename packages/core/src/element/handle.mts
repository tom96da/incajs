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
  checkSelector,
  closestNode,
  createNode,
  getAttribute,
  inputValue,
  matchesNode,
  queryNode,
  queryNodes,
  rootNodeId,
  scrollIntoViewNode,
  scrollOf,
  setAttribute,
  setInputValue,
  setStyle,
  textContent,
} from "../tree.mts";
import { attributeMembers, attributeText, requireArguments } from "./attributes.mts";
import { nodeListOf } from "./collections.mts";
import { elementTreeMembers } from "./elementTree.mts";
import { withEventHandlers } from "./eventHandlers.mts";
import { layoutBox, offsetOf, offsetParentOf, rectOf, tabIndexOf } from "./geometry.mts";
import { elementOf, isDestroyed, isRemoved, registerNode, withNodeId } from "./lifecycle.mts";
import { nodeMutationMembers } from "./nodeMutation.mts";
import { nodeRelationMembers } from "./nodeRelations.mts";
import { nodeTreeMembers } from "./nodeTree.mts";
import { scrollIntoViewArgs, scrollPosition } from "./scroll.mts";
import { characterDataMembers, createIncaText, nullableText, setElementText } from "./text.mts";
import type { NodeId, TagName } from "../types.mts";
import type { AttributeMembers } from "./attributes.mts";
import type { ElementTreeMembers } from "./elementTree.mts";
import type { HandleBuilders } from "./nodeMutation.mts";
import type { ScrollArgs, ScrollIntoViewOptions } from "./scroll.mts";
import type {
  EventHandlers,
  IncaElement,
  IncaFragment,
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

/** The builders a clone uses for each kind of handle. */
const builders: HandleBuilders = {
  element: createIncaElement,
  text: createIncaText,
  fragment: createIncaFragment,
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
    | keyof ElementTreeMembers
    | keyof NodeMembers
    | keyof TextContentMember
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
      return found === null ? null : elementOf(found)!;
    },
    querySelector: (...args: [selectors: string]) => {
      const text = selectorText(args, "querySelector");
      if (isDestroyed(element)) {
        checkSelector(text);
        return null;
      }
      const found = queryNode(id, text);
      return found === null ? null : elementOf(found)!;
    },
    querySelectorAll: (...args: [selectors: string]) => {
      const text = selectorText(args, "querySelectorAll");
      if (isDestroyed(element)) {
        checkSelector(text);
        return nodeListOf(() => []);
      }
      const found = queryNodes(id, text).map((node) => elementOf(node)!);
      return nodeListOf(() => found);
    },
  };
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
  Object.defineProperties(element, Object.getOwnPropertyDescriptors(nodeTreeMembers(element)));
  Object.defineProperties(
    element,
    Object.getOwnPropertyDescriptors(nodeMutationMembers(element, builders)),
  );
  Object.defineProperties(element, Object.getOwnPropertyDescriptors(nodeRelationMembers(element)));
  Object.defineProperties(
    element,
    Object.getOwnPropertyDescriptors(
      kind === "comment" ? characterDataMembers(element) : elementTextContent(element),
    ),
  );
  registerNode(element);
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
    nodeTreeMembers(fragment),
    nodeMutationMembers(fragment, builders),
    nodeRelationMembers(fragment),
    textMember,
  ]) {
    Object.defineProperties(fragment, Object.getOwnPropertyDescriptors(members));
  }
  registerNode(fragment);
  return fragment;
}
