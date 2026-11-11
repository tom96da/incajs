// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { classList as listOf, getAttribute, setAttribute } from "../tree.mts";
import { attributeText } from "./attributes.mts";
import { isDestroyed } from "./lifecycle.mts";
import type { NodeId } from "../types.mts";
import type { IncaElement } from "./types.mts";

/** The members {@link classMembers} builds. */
export type ClassMembers = Pick<IncaElement, "className" | "classList">;

/**
 * Builds `className` and `classList` of an element. The list is created on the first read and is
 * the same object afterwards.
 * @param id - the native node's id
 * @param element - the handle the members belong to
 * @returns an object that holds the two members
 */
export function classMembers(id: NodeId, element: IncaElement): ClassMembers {
  let list: DOMTokenList | undefined;
  const members = {
    get className() {
      return isDestroyed(element) ? "" : String(getAttribute(id, "class") ?? "");
    },
    set className(value: string) {
      const text = attributeText(value);
      if (!isDestroyed(element)) setAttribute(id, "class", text);
    },
    get classList(): DOMTokenList {
      return (list ??= listOf(id));
    },
    set classList(value: string) {
      (list ??= listOf(id)).value = value;
    },
  };
  // The setter takes text and the getter returns the list, which a single property type cannot say.
  return members as unknown as ClassMembers;
}
