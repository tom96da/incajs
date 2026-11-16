// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  attributeKey,
  attributeNames,
  getAttribute as readAttribute,
  removeAttribute as dropAttribute,
  setAttribute as writeAttribute,
} from "../ffi/index.mts";
import { requireArguments, attributeText } from "./arguments.mts";
import { namedNodeMapOf } from "./collections.mts";
import { isDestroyed, partsOf, receiverNode, receiverOf, ELEMENT_RULE } from "./lifecycle.mts";
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
function storeOf(element: IncaElement): AttributeStore {
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
function attributesOf(self: unknown): { element: IncaElement; store: AttributeStore } {
  const state = receiverOf(self, ELEMENT_RULE);
  const element = self as IncaElement;
  return { element, store: (state.attributes ??= storeOf(element)) as AttributeStore };
}

/**
 * Builds the attribute members and `id` of `Element`. A node the host freed reads no attribute and
 * ignores writes.
 * @returns the members, with `attributes` and `id` as accessors
 */
export function attributeMembers(): AttributeMembers & Pick<IncaElement, "id"> {
  return {
    get id() {
      const node = receiverNode(this, ELEMENT_RULE);
      return isDestroyed(node) ? "" : String(readAttribute(node.__node_id, "id") ?? "");
    },
    set id(value: string) {
      const node = receiverNode(this, ELEMENT_RULE);
      if (!isDestroyed(node)) writeAttribute(node.__node_id, "id", String(value));
    },
    getAttribute(qualifiedName: string) {
      const { element, store } = attributesOf(this);
      requireArguments(arguments, 1, "getAttribute");
      return isDestroyed(element) ? null : store.read(attributeText(qualifiedName));
    },
    setAttribute(qualifiedName: string, value: string) {
      const { element } = attributesOf(this);
      requireArguments(arguments, 2, "setAttribute");
      const name = attributeText(qualifiedName);
      const text = attributeText(value);
      if (!isDestroyed(element)) writeAttribute(element.__node_id, name, text);
    },
    hasAttribute(qualifiedName: string) {
      const { element, store } = attributesOf(this);
      requireArguments(arguments, 1, "hasAttribute");
      return !isDestroyed(element) && store.read(attributeText(qualifiedName)) !== null;
    },
    removeAttribute(qualifiedName: string) {
      const { element } = attributesOf(this);
      requireArguments(arguments, 1, "removeAttribute");
      const name = attributeText(qualifiedName);
      if (!isDestroyed(element)) dropAttribute(element.__node_id, name);
    },
    getAttributeNames() {
      return attributesOf(this).store.names();
    },
    get attributes() {
      return attributesOf(this).store.map();
    },
  };
}

/** The four name members of an element. */
type NameMembers = Pick<IncaElement, "tagName" | "localName" | "namespaceURI" | "prefix">;

/**
 * Builds the name members of `Element`, read from the parts the record computed once.
 * @returns the members
 */
export function nameMembers(): NameMembers {
  return {
    get localName() {
      return partsOf(receiverOf(this, ELEMENT_RULE))[0];
    },
    get namespaceURI() {
      return partsOf(receiverOf(this, ELEMENT_RULE))[1];
    },
    get prefix() {
      return partsOf(receiverOf(this, ELEMENT_RULE))[2];
    },
    get tagName() {
      return partsOf(receiverOf(this, ELEMENT_RULE))[3];
    },
  };
}
