// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { classList as listOf } from "../ffi/index.mts";
import { receiverOf, ELEMENT_RULE } from "./lifecycle.mts";
import type { DOMTokenList, Element } from "./types.mts";

/** The members {@link classMembers} builds. */
export type ClassMembers = Pick<Element, "classList">;

/**
 * Builds `classList` of `Element`. The list is created on the first read and is
 * the same object afterwards.
 * @returns an object that holds the member
 */
export function classMembers(): ClassMembers {
  const members = {
    get classList(): DOMTokenList {
      const state = receiverOf(this, ELEMENT_RULE);
      return (state.classList ??= listOf((this as unknown as Element).__node_id));
    },
    set classList(value: string) {
      const state = receiverOf(this, ELEMENT_RULE);
      (state.classList ??= listOf((this as unknown as Element).__node_id)).value = value;
    },
  };
  // The setter takes text and the getter returns the list, which a single property type cannot say.
  return members as unknown as ClassMembers;
}
