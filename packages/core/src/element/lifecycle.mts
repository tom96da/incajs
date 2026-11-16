// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import {
  HTML_NAMESPACE,
  holdNode,
  interfacePrototypes,
  registerUncloneable,
  releaseNode,
} from "../ffi/index.mts";
import { createIdTable } from "../idTable.mts";
import type { ElementName, EventTargetId, InterfacePrototypes } from "../ffi/index.mts";
import type { IncaDocument, NodeId } from "../types.mts";
import type {
  CSSStyleDeclaration,
  DOMStringMap,
  DOMTokenList,
  HTMLCollectionOf,
  IncaComment,
  IncaElement,
  IncaFragment,
  IncaHTMLElement,
  IncaNode,
  IncaText,
  NodeListOf,
} from "./types.mts";

/**
 * The functions that build and find the handles of native nodes. Every handle passes `text`,
 * `document` and `node`. A member that builds a node also passes `element`, `comment`,
 * `fragment`, `createText` and `createComment`: the first three build the handle of a host node
 * that exists, and the last two create a new host node and build its handle.
 */
export interface HandleBuilders {
  /** The handle of an element. A tag string gives an HTML element. */
  element?: {
    (id: NodeId, name?: string): IncaHTMLElement;
    (id: NodeId, name?: ElementName): IncaElement;
  };
  /** The handle of a text node the host has. */
  text: (id: NodeId, data: string) => IncaText;
  /** The handle of a comment the host has. */
  comment?: (id: NodeId) => IncaComment;
  /** Creates a host text node and returns its handle. */
  createText?: (data: string) => IncaText;
  /** Creates a host comment and returns its handle. */
  createComment?: (data: string) => IncaComment;
  fragment?: (id: NodeId) => IncaFragment;
  /** The handle of the document. */
  document: () => IncaDocument;
  /** The handle of a copy of the document, which has the node id of the copy. */
  documentCopy: (id: NodeId) => IncaDocument;
  /**
   * The handle of a node by id: the live handle, or the document handle for the document id. It
   * rebuilds the handle of the root container when that handle was collected.
   */
  node: (id: NodeId) => IncaNode | undefined;
}

const SVG_NAMESPACE = "http://www.w3.org/2000/svg";
const MATHML_NAMESPACE = "http://www.w3.org/1998/Math/MathML";

/** The prototypes of the host, read at the first call. */
let stored: InterfacePrototypes | undefined;

/**
 * The prototypes of the node interfaces. Core reads them here and never from the globals, so a
 * script that overwrites a global leaves the handles intact.
 * @returns the prototypes by interface name
 */
export function prototypes(): InterfacePrototypes {
  return (stored ??= interfacePrototypes());
}

/**
 * Picks the prototype of a new handle.
 * @param kind - the kind of the handle
 * @param name - the name of an element, an HTML `div` when omitted
 * @returns the prototype: `HTMLElement` for an element of the HTML namespace, the foreign element
 *   prototype for the SVG and MathML namespaces, `Element` for any other namespace, and the
 *   prototype of the interface that names each other kind
 */
export function prototypeOf(kind: IncaNode["kind"], name: ElementName = "div"): object {
  const all = prototypes();
  switch (kind) {
    case "comment":
      return all.Comment;
    case "text":
      return all.Text;
    case "fragment":
      return all.DocumentFragment;
    case "document":
      return all.HTMLDocument;
    case "element":
      switch (typeof name === "string" ? HTML_NAMESPACE : name[1]) {
        case HTML_NAMESPACE:
          return all.HTMLElement;
        case SVG_NAMESPACE:
        case MATHML_NAMESPACE:
          return all.ForeignElement;
        default:
          return all.Element;
      }
  }
}

/** The kind of a handle record: a node kind, or `"target"` for an event target with no node. */
export type RecordKind = "element" | "comment" | "text" | "fragment" | "document" | "target";

/** The bit of each record kind, so a receiver rule tests its kinds with one mask. */
const KIND_BITS: Record<RecordKind, number> = {
  element: 1,
  text: 2,
  comment: 4,
  fragment: 8,
  document: 16,
  target: 32,
};

/** The namespace class of an HTML element. */
const NS_HTML = 1;
/** The namespace class of an SVG or MathML element. */
const NS_FOREIGN = 2;
/** The namespace class of an element in any other namespace or in none. */
const NS_OTHER = 4;
/** Every namespace class: the value of a record that is no element. */
const NS_ALL = 7;

/** The name parts of an element: the local name, namespace, prefix and tag name. */
export type ElementParts = [
  localName: string,
  namespaceURI: string | null,
  prefix: string | null,
  tagName: string,
];

/**
 * The state of one handle. The cached members are filled at the first read and kept. The record is
 * reachable only through the brand map entry of its handle.
 */
export interface NodeState {
  kind: RecordKind;
  /** The bit of `kind`. */
  bit: number;
  /** The bit of the namespace class of an element, and all bits for any other record. */
  ns: number;
  /** The event names whose listeners are passive by default. */
  passive: readonly string[];
  /** The name of an element as the host gave it. */
  name?: ElementName;
  /** The name parts of an element, computed once. */
  parts?: ElementParts;
  /** Whether a document is a copy. */
  copy?: boolean;
  /** The target of a record of the kind `target`: an id, or a function that gives it. */
  target?: EventTargetId | (() => EventTargetId);
  classList?: DOMTokenList;
  style?: CSSStyleDeclaration;
  dataset?: DOMStringMap;
  /** The attribute store of an element, owned by `attributes.mts`. */
  attributes?: unknown;
  childNodes?: NodeListOf<IncaNode>;
  children?: HTMLCollectionOf<IncaElement>;
  /** The handler slots by event name, owned by `eventHandlers.mts`. */
  handlers?: Map<string, unknown>;
}

/** The brand map: every handle and event target, with its state record. */
const brand = new WeakMap<object, NodeState>();
registerUncloneable(brand);

/** The passive events of a handle that has none, shared so a new handle allocates no list. */
export const NO_EVENTS: readonly string[] = [];

/**
 * Reads the name parts of an element, computed at the first read. The tag name of an HTML element
 * is its name in ASCII upper case.
 * @param state - the record of an element
 * @returns the four parts
 */
export function partsOf(state: NodeState): ElementParts {
  if (state.parts === undefined) {
    const name = state.name!;
    state.parts =
      typeof name === "string"
        ? [name, HTML_NAMESPACE, null, name.replace(/[a-z]/g, (letter) => letter.toUpperCase())]
        : name;
  }
  return state.parts;
}

/**
 * Gives `handle` a read-only, non-enumerable `__node_id` and records its state.
 * @param handle - the new handle
 * @param id - the native node's id
 * @param kind - the kind of the handle
 * @param passive - the event names whose listeners are passive by default
 * @param name - the name of an element as the host gave it
 * @returns `handle`
 */
export function withState<T extends object>(
  handle: T,
  id: NodeId,
  kind: RecordKind,
  passive: readonly string[],
  name?: ElementName,
): T & { readonly __node_id: NodeId } {
  const space = name === undefined ? null : typeof name === "string" ? HTML_NAMESPACE : name[1];
  brand.set(handle, {
    kind,
    bit: KIND_BITS[kind],
    ns:
      name === undefined
        ? NS_ALL
        : space === HTML_NAMESPACE
          ? NS_HTML
          : space === SVG_NAMESPACE || space === MATHML_NAMESPACE
            ? NS_FOREIGN
            : NS_OTHER,
    passive,
    name,
  });
  return Object.defineProperty(handle, "__node_id", { value: id }) as T & {
    readonly __node_id: NodeId;
  };
}

/**
 * Records the state of an event target that is no node.
 * @param target - the new target
 * @param id - the target id, or a function that gives it
 * @param passive - the event names whose listeners are passive by default
 * @returns `target`
 */
export function withTargetState<T extends object>(
  target: T,
  id: EventTargetId | (() => EventTargetId),
  passive: readonly string[],
): T {
  brand.set(target, { kind: "target", bit: KIND_BITS.target, ns: NS_ALL, passive, target: id });
  return target;
}

/**
 * Reads the state record of a handle.
 * @param value - any value
 * @returns the record, or `undefined` for a value that is no handle
 */
export function stateOf(value: unknown): NodeState | undefined {
  return typeof value === "object" && value !== null ? brand.get(value) : undefined;
}

/** What a member needs of its receiver: the record kinds, the namespace classes and the name. */
export interface Receiver {
  readonly kinds: number;
  readonly ns: number;
  readonly name: string;
}

/** The kinds of the records that are nodes. */
const NODE_KINDS =
  KIND_BITS.element | KIND_BITS.text | KIND_BITS.comment | KIND_BITS.fragment | KIND_BITS.document;

/** The receiver of a `Node` member: any node. */
export const NODE_RULE: Receiver = { kinds: NODE_KINDS, ns: NS_ALL, name: "a Node" };
/** The receiver of a `CharacterData` member: a text node or a comment. */
export const CHARACTER_DATA_RULE: Receiver = {
  kinds: KIND_BITS.text | KIND_BITS.comment,
  ns: NS_ALL,
  name: "a CharacterData",
};
/** The receiver of a `Text` member: a text node. */
export const TEXT_RULE: Receiver = { kinds: KIND_BITS.text, ns: NS_ALL, name: "a Text" };
/** The receiver of an `Element` member: an element of any namespace. */
export const ELEMENT_RULE: Receiver = { kinds: KIND_BITS.element, ns: NS_ALL, name: "an Element" };
/** The receiver of an `HTMLElement` member: an element of the HTML namespace. */
export const HTML_ELEMENT_RULE: Receiver = {
  kinds: KIND_BITS.element,
  ns: NS_HTML,
  name: "an HTMLElement",
};
/** The receiver of a shared element member: an HTML, SVG or MathML element. */
export const HTML_OR_SVG_RULE: Receiver = {
  kinds: KIND_BITS.element,
  ns: NS_HTML | NS_FOREIGN,
  name: "an HTML, SVG or MathML element",
};
/** The receiver of a `ParentNode` member: an element, a fragment or the document. */
export const PARENT_NODE_RULE: Receiver = {
  kinds: KIND_BITS.element | KIND_BITS.fragment | KIND_BITS.document,
  ns: NS_ALL,
  name: "an Element, Document or DocumentFragment",
};
/** The receiver of a sibling member: an element, a text node or a comment. */
export const NON_DOCUMENT_CHILD_RULE: Receiver = {
  kinds: KIND_BITS.element | KIND_BITS.text | KIND_BITS.comment,
  ns: NS_ALL,
  name: "an Element or CharacterData",
};
/** The receiver of a `Document` member: the document. */
export const DOCUMENT_RULE: Receiver = {
  kinds: KIND_BITS.document,
  ns: NS_ALL,
  name: "a Document",
};
/** The receiver of a listener method: any node or event target. */
export const EVENT_TARGET_RULE: Receiver = {
  kinds: NODE_KINDS | KIND_BITS.target,
  ns: NS_ALL,
  name: "an EventTarget",
};
/** The receiver of the handler properties of `window`: an event target that is no node. */
export const TARGET_RULE: Receiver = {
  kinds: KIND_BITS.target,
  ns: NS_ALL,
  name: "an EventTarget",
};

/**
 * Builds the error of a wrong receiver.
 * @param rule - the rule the receiver failed
 * @returns the `TypeError`
 */
function receiverError(rule: Receiver): TypeError {
  return new TypeError(`the receiver is not ${rule.name}. call the method on an instance`);
}

/**
 * Checks the receiver of a member.
 * @param value - the receiver, `this` of the member
 * @param rule - the rule of the interface the member belongs to
 * @returns the state record of `value`
 * @throws {TypeError} when `value` is no handle of the interface
 */
export function receiverOf(value: unknown, rule: Receiver): NodeState {
  const state = typeof value === "object" && value !== null ? brand.get(value) : undefined;
  if (state === undefined || (rule.kinds & state.bit) === 0 || (rule.ns & state.ns) === 0) {
    throw receiverError(rule);
  }
  return state;
}

/**
 * Checks the receiver of a member like {@link receiverOf} and gives the receiver itself.
 * @param value - the receiver, `this` of the member
 * @param rule - the rule of the interface the member belongs to
 * @returns `value`, as the handle type `T`
 * @throws {TypeError} when `value` is no handle of the interface
 */
export function receiverNode<T extends IncaNode = IncaNode>(value: unknown, rule: Receiver): T {
  receiverOf(value, rule);
  return value as T;
}

/**
 * Whether `value` is a node handle.
 * @param value - the value to check
 * @returns `true` for an element, a comment, a text handle, a fragment or the document
 */
export function isNode(value: unknown): value is IncaNode {
  const state = stateOf(value);
  return state !== undefined && state.kind !== "target";
}

/**
 * The handle of each node that entered a parent and was not removed since. The entry keeps the
 * handle alive.
 */
const strong = createIdTable<IncaNode>();

/**
 * A weak reference to the handle of every other node: a node with no parent, a node below a
 * removed node, and the root container.
 */
const weak = createIdTable<WeakRef<IncaNode>>();

/** The id of the root container, which stays weak while it sits under the document. */
let rootId: NodeId | undefined;

/**
 * Records `node` under its id, weakly. A handle already recorded under the id is replaced.
 * @param node - the handle to record
 */
export function registerNode(node: IncaNode): void {
  strong.delete(node.__node_id);
  weak.set(node.__node_id, new WeakRef(node));
}

/**
 * Records the handle of the root container. The container keeps its weak entry when it enters
 * the tree, so the handle of an unmounted app is collectable.
 * @param node - the root container's handle
 */
export function registerRoot(node: IncaElement): void {
  rootId = node.__node_id;
  registerNode(node);
}

/**
 * Makes the handle of `node` strong, so it lives while the node has a parent. The handles below a
 * removed node keep their weak entries when the node enters a parent again: the node's handle
 * keeps them alive. The root container stays weak.
 * @param node - the handle of a node the host put under a parent
 */
export function entered(node: IncaNode): void {
  const id = node.__node_id;
  if (id === rootId) return;
  strong.set(id, node);
  weak.delete(id);
}

/**
 * Makes the handle of a node that has no parent weak, and holds the node until the handle is
 * collected. The entries below the node do not change.
 * @param node - the handle of a node the host took out of its parent
 */
export function left(node: IncaNode): void {
  unparent(node);
  hold(node);
}

/**
 * The handles `leftHeld` recorded below each removed node at its last removal, which live as long as
 * the handle of that node. A handle moved out of the subtree later stays in the list until the top
 * is collected. The host frees a removed subtree at a frame after the handle of its top is
 * collected.
 */
const kept = new WeakMap<IncaNode, IncaNode[]>();

/**
 * Makes the handle of a node that has no parent weak, for a node the host holds already. Every
 * live handle below it becomes weak too and lives as long as `node`'s handle, whether the entry
 * was strong or weak before. The release of the node registers on the collection of the handle.
 * @param node - the handle of a node that `removeNode` took out of its parent
 * @param nodeIds - the ids of `node` and every node below it
 */
export function leftHeld(node: IncaNode, nodeIds: readonly NodeId[]): void {
  unparent(node);
  const below: IncaNode[] = [];
  for (const id of nodeIds) {
    const handle = nodeOf(id);
    if (handle === undefined || handle === node) continue;
    unparent(handle);
    below.push(handle);
  }
  kept.set(node, below);
  if (!isHeld(node)) releaseWhenCollected(node);
}

/**
 * Moves the entry of `node` to the weak table.
 * @param node - the handle of a node that has no parent, or sits below a removed node
 */
function unparent(node: IncaNode): void {
  const id = node.__node_id;
  weak.set(id, new WeakRef(node));
  strong.delete(id);
}

/**
 * Holds `node` until its handle is collected, so the host does not free it at the next frame. A
 * node held already changes nothing.
 * @param node - the handle of a node that has no parent
 */
export function hold(node: IncaNode): void {
  if (isHeld(node)) return;
  holdNode(node.__node_id);
  releaseWhenCollected(node);
}

/**
 * Looks up the live handle of a native node of any kind.
 * @param id - the native node's id
 * @returns the handle, or `undefined` when none is live
 */
export function nodeOf(id: NodeId): IncaNode | undefined {
  return strong.get(id) ?? weak.get(id)?.deref();
}

/**
 * Looks up the live handle of a native element.
 * @param id - the native node's id
 * @returns the handle, or `undefined` when none is live or the node is not an element
 */
export function elementOf(id: NodeId): IncaElement | undefined {
  const node = nodeOf(id);
  return node?.kind === "element" ? node : undefined;
}

/** The content handle of each template handle. */
const contents = new WeakMap<IncaElement, IncaFragment>();

/**
 * The template handle of each content handle. The entry keeps the template reachable while its
 * content is, so the host holds both nodes as long as either handle is.
 */
const owners = new WeakMap<IncaFragment, IncaElement>();

/**
 * Reads the content handle of a template that already has one.
 * @param node - any node handle
 * @returns the stored content, or `undefined`
 */
export function existingContent(node: IncaNode): IncaFragment | undefined {
  return node.kind === "element" ? contents.get(node) : undefined;
}

/**
 * Links a template handle and its content handle both ways.
 * @param template - the template handle
 * @param content - its content handle
 */
export function storeContent(template: IncaElement, content: IncaFragment): void {
  contents.set(template, content);
  owners.set(content, template);
}

const removed = new WeakSet<IncaNode>();

/**
 * Whether `node` left the tree for good, removed or freed. A removed node stays in the host and
 * its members read real values. Its listeners are gone.
 * @param node - the handle to check
 * @returns `true` after {@link markRemoved} reached `node`, or when the host freed it
 */
export function isRemoved(node: IncaNode): boolean {
  return removed.has(node) || isDestroyed(node);
}

/**
 * Records that the nodes of a removed subtree left the tree. An id with no live handle is
 * skipped.
 * @param nodeIds - the ids of the removed node and every node below it
 */
export function markRemoved(nodeIds: readonly NodeId[]): void {
  for (const id of nodeIds) {
    const node = nodeOf(id);
    if (node) removed.add(node);
  }
}

/**
 * Whether the host freed `node` with its subtree.
 * @param node - the handle to check
 * @returns `true` when no table holds `node` under its id
 */
export function isDestroyed(node: IncaNode): boolean {
  return nodeOf(node.__node_id) !== node;
}

/**
 * Drops the table entries of the nodes the host freed. The ids already list every node of a freed
 * subtree.
 * @param nodeIds - ids of nodes the host freed, subtrees included
 * @internal
 */
export function markFreed(nodeIds: readonly NodeId[]): void {
  for (const id of nodeIds) {
    strong.delete(id);
    weak.delete(id);
  }
}

/**
 * Releases the hold of a node whose handle was collected. A node destroyed
 * already is unknown to the host and the call does nothing.
 */
const released = new FinalizationRegistry<NodeId>((id) => releaseNode(id));

const held = new WeakSet<IncaNode>();

/**
 * Whether {@link releaseWhenCollected} already covers `handle`.
 * @param handle - the handle to check
 * @returns `true` after `releaseWhenCollected(handle)`
 * @internal
 */
export function isHeld(handle: IncaNode): boolean {
  return held.has(handle);
}

/**
 * Releases the hold of `handle`'s node once the handle is collected.
 * @param handle - the handle of a node held with `holdNode`
 * @internal
 */
export function releaseWhenCollected(handle: IncaNode): void {
  held.add(handle);
  released.register(handle, handle.__node_id);
}
