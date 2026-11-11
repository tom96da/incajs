// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  attributeNames,
  getAttribute as readAttribute,
  removeAttribute as dropAttribute,
  setAttribute as writeAttribute,
} from "../tree.mts";
import { namedNodeMapOf } from "./collections.mts";
import { isDestroyed } from "./lifecycle.mts";
import type { NodeId } from "../types.mts";
import type { Attr, IncaElement, NamedNodeMap } from "./types.mts";

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

/** Folds the letters A to Z of an attribute name to lower case. Every other character stays. */
function foldName(name: string): string {
  return name.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
}

/**
 * An attribute that reads its value from the element on each access. It detaches when the element
 * no longer holds it, and then keeps the last value it read.
 */
class AttributeNode implements Attr {
  readonly #name: string;
  readonly #id: NodeId;
  readonly #forget: (attr: AttributeNode) => void;
  #owner: IncaElement | null;
  #value: string;

  constructor(
    name: string,
    value: string,
    id: NodeId,
    owner: IncaElement,
    forget: (attr: AttributeNode) => void,
  ) {
    this.#name = name;
    this.#value = value;
    this.#id = id;
    this.#owner = owner;
    this.#forget = forget;
  }

  /** Detaches `attr` from its element and from the element's cache. */
  static detach(attr: AttributeNode): void {
    attr.#owner = null;
    attr.#forget(attr);
  }

  /** Reads the element, and detaches when the attribute is gone. Returns whether it is attached. */
  #attached(): boolean {
    if (this.#owner === null) return false;
    const value = isDestroyed(this.#owner) ? null : readAttribute(this.#id, this.#name);
    if (value === null) {
      AttributeNode.detach(this);
      return false;
    }
    this.#value = attributeText(value);
    return true;
  }

  get name(): string {
    return this.#name;
  }

  get localName(): string {
    return this.#name;
  }

  get value(): string {
    this.#attached();
    return this.#value;
  }

  set value(value: unknown) {
    const text = attributeText(value);
    if (this.#attached()) writeAttribute(this.#id, this.#name, text);
    this.#value = text;
  }

  get namespaceURI(): null {
    return null;
  }

  get prefix(): null {
    return null;
  }

  get specified(): true {
    return true;
  }

  get ownerElement(): IncaElement | null {
    this.#attached();
    return this.#owner;
  }

  get [Symbol.toStringTag](): string {
    return "Attr";
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
  const held = new Map<string, AttributeNode>();
  const forget = (attr: AttributeNode): void => {
    if (held.get(attr.name) === attr) held.delete(attr.name);
  };
  const attrOf = (name: string, value: string): AttributeNode => {
    let attr = held.get(name);
    if (!attr) {
      attr = new AttributeNode(name, value, id, element, forget);
      held.set(name, attr);
    }
    return attr;
  };
  /** The attribute `name`, which is already in the host's lower case. */
  const find = (name: string): Attr | null => {
    const value = isDestroyed(element) ? null : read(name);
    if (value !== null) return attrOf(name, value);
    const gone = held.get(name);
    if (gone) AttributeNode.detach(gone);
    return null;
  };
  const list = (): Attr[] => {
    const present = names();
    const set = new Set(present);
    for (const [name, attr] of held) if (!set.has(name)) AttributeNode.detach(attr);
    return present.map((name) => held.get(name) ?? attrOf(name, read(name)!));
  };
  let map: NamedNodeMap | undefined;
  return {
    getAttribute: (...args: [name: string]) => {
      requireArguments(args, 1, "getAttribute");
      return isDestroyed(element) ? null : read(attributeText(args[0]));
    },
    setAttribute: (...args: [name: string, value: string]) => {
      requireArguments(args, 2, "setAttribute");
      const name = attributeText(args[0]);
      const value = attributeText(args[1]);
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
      map ??= namedNodeMapOf(
        list,
        (name) => find(foldName(name)),
        (key) => (key === foldName(key) ? find(key) : null),
      );
      return map;
    },
  };
}
