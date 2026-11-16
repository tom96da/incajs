// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { styleOf } from "../ffi/index.mts";
import { attributeText } from "./arguments.mts";
import { HTML_OR_SVG_RULE, receiverOf } from "./lifecycle.mts";
import type { CSSStyleDeclaration, IncaHTMLElement } from "./types.mts";

/** The member {@link styleMembers} builds. */
export type StyleMembers = Pick<IncaHTMLElement, "style">;

/**
 * Builds the `style` member of the shared element members. The block is created on the first read
 * and kept, so every read returns one object. Assigning writes `cssText`, with `null` as the empty
 * string.
 * @returns the member
 */
export function styleMembers(): StyleMembers {
  const read = (self: unknown): CSSStyleDeclaration => {
    const state = receiverOf(self, HTML_OR_SVG_RULE);
    return (state.style ??= styleOf((self as IncaHTMLElement).__node_id));
  };
  const members = {
    get style(): CSSStyleDeclaration {
      return read(this);
    },
    set style(value: string | null) {
      read(this).cssText = value === null ? "" : attributeText(value);
    },
  };
  // The setter takes text and the getter returns the block; one property type cannot say both.
  return members as unknown as StyleMembers;
}
