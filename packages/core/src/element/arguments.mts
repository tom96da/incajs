// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The checks and conversions the members apply to the arguments of a call.

import { isNode } from "./lifecycle.mts";
import type { HandleNode } from "./handleNode.mts";

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
export function requireArguments(args: ArrayLike<unknown>, count: number, method: string): void {
  if (args.length < count) {
    throw new TypeError(`${method}() needs ${count} argument${count === 1 ? "" : "s"}`);
  }
}

/**
 * Converts an assignment to a nullable text member: `null` and `undefined` store `""`.
 * @param value - the assigned value
 * @returns the text to store
 */
export function nullableText(value: unknown): string {
  return value === null || value === undefined ? "" : attributeText(value);
}

/**
 * Checks that an argument is a node.
 * @param value - the argument
 * @param position - the argument's place in the call, counted from 1
 * @returns the node
 * @throws {TypeError} when `value` is not a node
 * @internal
 */
export function nodeArgument(value: unknown, position: number): HandleNode {
  if (!isNode(value)) {
    throw new TypeError(
      `parameter ${position} is not a node. pass an element, a text node, a comment or a fragment`,
    );
  }
  return value;
}
