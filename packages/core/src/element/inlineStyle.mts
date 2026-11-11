// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { styleOf } from "../tree.mts";
import { attributeText } from "./attributes.mts";
import type { NodeId } from "../types.mts";
import type { CSSStyleDeclaration, IncaElement } from "./types.mts";

/** The member {@link styleMembers} builds. */
export type StyleMembers = Pick<IncaElement, "style">;

/**
 * Builds the `style` member of an element. The block is created on the first read and kept, so
 * every read returns one object. Assigning writes `cssText`, with `null` as the empty string.
 * @param id - the native node's id
 * @returns the member
 */
export function styleMembers(id: NodeId): StyleMembers {
  let block: CSSStyleDeclaration | undefined;
  const read = () => (block ??= styleOf(id));
  const members = {
    get style(): CSSStyleDeclaration {
      return read();
    },
    set style(value: string | null) {
      read().cssText = value === null ? "" : attributeText(value);
    },
  };
  // The setter takes text and the getter returns the block; one property type cannot say both.
  return members as unknown as StyleMembers;
}
