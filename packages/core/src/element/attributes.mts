// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  attributeKey,
  attributeNames,
  getAttribute as readAttribute,
  setAttribute as writeAttribute,
} from "../ffi/index.mts";
import { attributeText } from "./arguments.mts";
import { namedNodeMapOf } from "./collections.mts";
import { isDestroyed, receiverOf, ELEMENT_RULE } from "./lifecycle.mts";
import type { NodeId } from "../types.mts";
import type { Attr, Element, NamedNodeMap } from "./types.mts";

/** The members {@link attributeMembers} builds. */
export type AttributeMembers = Pick<Element, "attributes">;

/**
 * An attribute that reads its value from the element on each access. It detaches when the element
 * no longer holds it, and then keeps the last value it read.
 */
class AttributeNode implements Attr {
  readonly #name: string;
  readonly #id: NodeId;
  readonly #forget: (attr: AttributeNode) => void;
  #owner: Element | null;
  #value: string;

  constructor(
    name: string,
    value: string,
    id: NodeId,
    owner: Element,
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

  get ownerElement(): Element | null {
    this.#attached();
    return this.#owner;
  }

  get [Symbol.toStringTag](): string {
    return "Attr";
  }
}

/** The attributes of one element: the attribute objects it handed out and its live map. */
interface AttributeStore {
  names: () => string[];
  read: (name: string) => string | null;
  find: (name: string) => Attr | null;
  map: () => NamedNodeMap;
}

/**
 * Builds the attribute store of an element. A node the host freed reads no attribute.
 * @param element - the handle
 * @returns the store
 */
function storeOf(element: Element): AttributeStore {
  const id = element.__node_id;
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
  /** The attribute `name`, looked up under the key the host gives it. */
  const find = (name: string): Attr | null => {
    if (isDestroyed(element)) return null;
    const key = attributeKey(id, name);
    const value = read(key);
    if (value !== null) return attrOf(key, value);
    const gone = held.get(key);
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
  return { names, read, find, map: () => (map ??= namedNodeMapOf(list, find)) };
}

/**
 * Reads the attribute store of the receiver, which is created at the first read.
 * @param self - `this` of the member
 * @returns the element and its store
 */
function attributesOf(self: unknown): { element: Element; store: AttributeStore } {
  const state = receiverOf(self, ELEMENT_RULE);
  const element = self as Element;
  return { element, store: (state.attributes ??= storeOf(element)) as AttributeStore };
}

/**
 * Builds `attributes` of `Element`. The host defines the other attribute members.
 * @returns the members, with `attributes` as an accessor
 */
export function attributeMembers(): AttributeMembers {
  return {
    get attributes() {
      return attributesOf(this).store.map();
    },
  };
}
