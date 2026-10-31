// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  attributeNames,
  getAttribute as readAttribute,
  removeAttribute as dropAttribute,
  setAttribute as writeAttribute,
} from "../tree.mts";
import { isDestroyed } from "./lifecycle.mts";
import type { NodeId } from "../types.mts";
import type { IncaElement } from "./types.mts";

/** The members {@link attributeMembers} builds. */
export type AttributeMembers = Pick<
  IncaElement,
  | "getAttribute"
  | "setAttribute"
  | "hasAttribute"
  | "removeAttribute"
  | "getAttributeNames"
  | "attributes"
>;

/** The characters an attribute name cannot hold. */
const INVALID_NAME_CHARACTERS = new Set(["\0", "\t", "\n", "\f", "\r", " ", "/", "=", ">"]);

/**
 * Whether `name` is a valid attribute local name: not empty, with no ASCII whitespace, NUL, `/`,
 * `=` or `>`.
 * @param name - the name to check
 * @returns `true` for a valid name
 */
function validName(name: string): boolean {
  if (name.length === 0) return false;
  for (const character of name) if (INVALID_NAME_CHARACTERS.has(character)) return false;
  return true;
}

/**
 * Converts a value to text. A `Symbol` throws a `TypeError`.
 * @param value - the value to convert
 * @returns the text of `value`
 */
export function attributeText(value: unknown): string {
  if (typeof value === "symbol") throw new TypeError("a symbol cannot become text");
  // oxlint-disable-next-line typescript/no-base-to-string
  return String(value);
}

/**
 * Throws a `TypeError` when a call passes fewer than `count` arguments.
 * @param args - the arguments the call received
 * @param count - the number the method needs
 * @param method - the method name for the message
 */
export function requireArguments(args: readonly unknown[], count: number, method: string): void {
  if (args.length < count) {
    throw new TypeError(`${method}() needs ${count} argument${count === 1 ? "" : "s"}`);
  }
}

/**
 * Builds the attribute members of the element `element` that the host knows as `id`. A node the
 * host freed reads no attribute and ignores writes.
 * @param id - the native node's id
 * @param element - the handle the members belong to
 * @returns the members, with `attributes` as a getter
 */
export function attributeMembers(id: NodeId, element: IncaElement): AttributeMembers {
  const read = (name: string): string | null => {
    const value = readAttribute(id, name);
    return value === null ? null : attributeText(value);
  };
  const names = (): string[] => (isDestroyed(element) ? [] : attributeNames(id));
  return {
    getAttribute: (...args: [name: string]) => {
      requireArguments(args, 1, "getAttribute");
      return isDestroyed(element) ? null : read(attributeText(args[0]));
    },
    setAttribute: (...args: [name: string, value: string]) => {
      requireArguments(args, 2, "setAttribute");
      const name = attributeText(args[0]);
      const value = attributeText(args[1]);
      if (!validName(name)) {
        throw new DOMException(
          "the attribute name is not valid. it is empty or holds whitespace, '/', '=' or '>'",
          "InvalidCharacterError",
        );
      }
      if (!isDestroyed(element)) writeAttribute(id, name, value);
    },
    hasAttribute: (...args: [name: string]) => {
      requireArguments(args, 1, "hasAttribute");
      return !isDestroyed(element) && read(attributeText(args[0])) !== null;
    },
    removeAttribute: (...args: [name: string]) => {
      requireArguments(args, 1, "removeAttribute");
      const name = attributeText(args[0]);
      if (!isDestroyed(element)) dropAttribute(id, name);
    },
    getAttributeNames: () => names(),
    get attributes() {
      return names().map((name) => ({ name, value: read(name) ?? "" }));
    },
  };
}
