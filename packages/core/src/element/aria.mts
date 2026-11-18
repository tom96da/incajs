// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The members of an element that name other elements by ID or hold elements assigned from script.
// The host keeps the ids, and a list member keeps its last array here.

import { ariaElements, setAriaElements } from "../ffi/index.mts";
import { ELEMENT_RULE, receiverNode, stateOf } from "./lifecycle.mts";
import type { HandleBuilders } from "./lifecycle.mts";
import type { Element } from "./types.mts";

/** The element member that holds one element, and the attribute it reflects. */
const SINGLE = ["ariaActiveDescendantElement", "aria-activedescendant"] as const;

/** The element members that hold a list, with the attributes they reflect. */
const LISTS = [
  ["ariaControlsElements", "aria-controls"],
  ["ariaDescribedByElements", "aria-describedby"],
  ["ariaDetailsElements", "aria-details"],
  ["ariaErrorMessageElements", "aria-errormessage"],
  ["ariaFlowToElements", "aria-flowto"],
  ["ariaLabelledByElements", "aria-labelledby"],
  ["ariaOwnsElements", "aria-owns"],
] as const;

/**
 * Whether `value` is the handle of an element.
 * @param value - any value
 * @returns `true` for an element
 */
function isElement(value: unknown): value is Element {
  return stateOf(value)?.kind === "element";
}

/**
 * Builds the element members of the ARIA table, for `Element`. A read asks the host again, so a
 * node that left the tree drops out. A list member returns the array of its last read while the
 * nodes and their order stay the same.
 * @param builders - the lookup of a handle by node id
 * @returns the members, as accessors
 */
export function ariaElementMembers(builders: HandleBuilders): object {
  const handles = (ids: readonly number[]): Element[] =>
    ids.flatMap((id) => (builders.node(id) as Element | undefined) ?? []);
  const members: object = {};
  const [single, singleAttribute] = SINGLE;
  Object.defineProperty(members, single, {
    get(this: unknown): Element | null {
      const node = receiverNode(this, ELEMENT_RULE);
      const [first] = ariaElements(node.__node_id, singleAttribute) ?? [];
      return first === undefined ? null : (handles([first])[0] ?? null);
    },
    set(this: unknown, value: unknown): void {
      const node = receiverNode(this, ELEMENT_RULE);
      if (value === null || value === undefined) {
        setAriaElements(node.__node_id, singleAttribute, null);
        return;
      }
      if (!isElement(value)) {
        throw new TypeError("the value is not an Element. pass an element or null");
      }
      setAriaElements(node.__node_id, singleAttribute, [value.__node_id]);
    },
    enumerable: true,
    configurable: true,
  });
  for (const [name, attribute] of LISTS) {
    Object.defineProperty(members, name, {
      get(this: unknown): readonly Element[] | null {
        const node = receiverNode(this, ELEMENT_RULE);
        const state = stateOf(node);
        const ids = ariaElements(node.__node_id, attribute);
        if (ids === null) {
          state?.aria?.delete(attribute);
          return null;
        }
        const cached = state?.aria?.get(attribute);
        if (cached?.ids.length === ids.length && cached.ids.every((id, i) => id === ids[i])) {
          return cached.array;
        }
        const array = Object.freeze(handles(ids));
        if (state !== undefined) (state.aria ??= new Map()).set(attribute, { ids, array });
        return array;
      },
      set(this: unknown, value: unknown): void {
        const node = receiverNode(this, ELEMENT_RULE);
        if (value === null || value === undefined) {
          setAriaElements(node.__node_id, attribute, null);
          return;
        }
        if (
          Object(value) !== value ||
          typeof (value as Iterable<unknown>)[Symbol.iterator] !== "function"
        ) {
          throw new TypeError(
            "the value is not a sequence of Element. pass an array of elements or null",
          );
        }
        const items = [...(value as Iterable<unknown>)];
        if (!items.every(isElement)) {
          throw new TypeError("the value is not an Element. pass an element or null");
        }
        setAriaElements(
          node.__node_id,
          attribute,
          items.map((item) => item.__node_id),
        );
      },
      enumerable: true,
      configurable: true,
    });
  }
  return members;
}
