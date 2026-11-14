// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { vi } from "vitest";

import { HTML_NAMESPACE } from "../src/ffi/index.mts";
import { nativeStub } from "./setup.mts";
import type { ElementEntry, HostCallback, NodeEntry } from "../src/ffi/index.mts";

/** The host object core calls. */
type Native = typeof globalThis.__inca_native__;

/** One node of the fake tree. */
interface FakeNode {
  tag: string;
  parent: number | null;
  children: number[];
  listeners: number[];
  /** The content fragment of a template. */
  content: number | null;
  /** The `value` attribute: the text of a text node or a comment. */
  data: string | null;
  /** The attributes by name. */
  attrs: Map<string, unknown>;
}

/** The fake host and what a test reads from it. */
export interface FakeHost {
  readonly DOCUMENT: number;
  readonly ROOT: number;
  /** The nodes by id. */
  readonly nodes: Map<number, FakeNode>;
  /** The nodes `holdNode` keeps. */
  readonly held: Set<number>;
  /** The function the host keeps for each registration. */
  readonly callbacks: Map<number, HostCallback>;
  /** The natives, which a test spreads into `nativeStub` after its own mocks. */
  readonly natives: Partial<Native>;
  /** Spies over the natives a test counts calls of. */
  readonly spies: {
    readonly linkedNode: ReturnType<typeof vi.fn<Native["linkedNode"]>>;
    readonly childNodeIds: ReturnType<typeof vi.fn<Native["childNodeIds"]>>;
    readonly removeNode: ReturnType<typeof vi.fn<Native["removeNode"]>>;
    readonly holdNode: ReturnType<typeof vi.fn<Native["holdNode"]>>;
    readonly releaseNode: ReturnType<typeof vi.fn<Native["releaseNode"]>>;
    readonly insertBefore: ReturnType<typeof vi.fn<Native["insertBefore"]>>;
    readonly removeChild: ReturnType<typeof vi.fn<Native["removeChild"]>>;
    readonly cloneNode: ReturnType<typeof vi.fn<Native["cloneNode"]>>;
  };
  /** Creates a node of the fake tree, as the native `createNode` does. */
  create(tag: string): number;
  /** The ids of the nodes with the tag. */
  idsOf(tag: string): number[];
  /** The nodes the next frame frees: no parent, not held, not the root or the document. */
  orphans(): number[];
  /** Frees the orphans with their subtrees, as the host does at a frame. */
  frame(): void;
}

/** The ids of the document and the mount container, the same in every fake host. */
const DOCUMENT_ID = 1;
const ROOT_ID = 2;

/** Ids stay unique across fake hosts, so a handle collected late frees no node of a later test. */
let nextId = 10;

/**
 * Builds a fake of the host tree with its orphan model: a created node has no parent, `removeNode`
 * holds the node it takes out, and `frame()` frees every parentless node nothing holds with its
 * subtree and reports the freed ids to `__inca_released__`. The document holds the root container.
 * @param extra - natives a test replaces
 * @returns the fake, installed as `__inca_native__`
 */
export function installFakeHost(extra: Partial<Native> = {}): FakeHost {
  const nodes = new Map<number, FakeNode>();
  const held = new Set<number>();
  const callbacks = new Map<number, HostCallback>();
  const DOCUMENT = DOCUMENT_ID;
  const ROOT = ROOT_ID;
  let nextCallbackId = 1;

  const create = (tag: string): number => {
    const id = nextId++;
    nodes.set(id, {
      tag,
      parent: null,
      children: [],
      listeners: [],
      content: null,
      data: null,
      attrs: new Map(),
    });
    if (tag === "template") {
      const content = create("#fragment");
      nodes.get(id)!.content = content;
    }
    return id;
  };
  nodes.set(DOCUMENT, {
    tag: "#document",
    parent: null,
    children: [ROOT],
    listeners: [],
    content: null,
    data: null,
    attrs: new Map(),
  });
  nodes.set(ROOT, {
    tag: "div",
    parent: DOCUMENT,
    children: [],
    listeners: [],
    content: null,
    data: null,
    attrs: new Map(),
  });

  const nodeOf = (id: number): FakeNode => {
    const node = nodes.get(id);
    if (!node) throw new TypeError(`unknown node id: ${id}`);
    return node;
  };
  const isElement = (id: number): boolean => !nodeOf(id).tag.startsWith("#");
  const siblingsOf = (id: number): number[] => {
    const parent = nodeOf(id).parent;
    return parent === null ? [] : nodeOf(parent).children;
  };
  const nextOf = (id: number): number | null =>
    siblingsOf(id)[siblingsOf(id).indexOf(id) + 1] ?? null;
  const detach = (id: number): void => {
    const node = nodeOf(id);
    if (node.parent === null) return;
    const siblings = nodeOf(node.parent).children;
    siblings.splice(siblings.indexOf(id), 1);
    node.parent = null;
  };
  const place = (parentId: number, nodeId: number, refId: number | null): number => {
    const parent = nodeOf(parentId);
    const node = nodeOf(nodeId);
    if (refId !== null && nodeOf(refId).parent !== parentId) {
      throw new DOMException("not a child", "NotFoundError");
    }
    const ref = refId === nodeId ? nextOf(nodeId) : refId;
    const moved = node.tag === "#fragment" ? [...node.children] : [nodeId];
    for (const id of moved) detach(id);
    const at = ref === null ? parent.children.length : parent.children.indexOf(ref);
    parent.children.splice(at, 0, ...moved);
    for (const id of moved) nodeOf(id).parent = parentId;
    return at;
  };
  const subtree = (id: number): number[] => {
    const node = nodes.get(id);
    if (!node) return [];
    const ids = [id];
    if (node.content !== null) ids.push(...subtree(node.content));
    for (const child of node.children) ids.push(...subtree(child));
    return ids;
  };
  const dropListeners = (ids: number[]): number[] => {
    const dropped: number[] = [];
    for (const id of ids) {
      const node = nodeOf(id);
      dropped.push(...node.listeners);
      for (const callbackId of node.listeners) callbacks.delete(callbackId);
      node.listeners = [];
    }
    return dropped;
  };
  const linked = (id: number, link: string, elementOnly: boolean): number | null => {
    const node = nodeOf(id);
    const wanted = (candidate: number): boolean => !elementOnly || isElement(candidate);
    if (link === "parent") {
      return node.parent !== null && wanted(node.parent) ? node.parent : null;
    }
    if (link === "first") return node.children.find(wanted) ?? null;
    if (link === "last") return [...node.children].reverse().find(wanted) ?? null;
    const siblings = siblingsOf(id);
    const at = siblings.indexOf(id);
    if (link === "next") return siblings.slice(at + 1).find(wanted) ?? null;
    return siblings.slice(0, at).reverse().find(wanted) ?? null;
  };
  const matching = (id: number, tag: string): number[] =>
    subtree(id).filter((found) => found !== id && nodeOf(found).tag === tag);

  const copyOf = (original: number, deep: boolean): (NodeEntry | ElementEntry)[] => {
    const { tag, content, children, data, attrs } = nodeOf(original);
    const made = create(tag);
    nodeOf(made).data = data;
    nodeOf(made).attrs = new Map(attrs);
    const entries: (NodeEntry | ElementEntry)[] = [
      tag.startsWith("#")
        ? [made, tag, tag === "#text" ? data : null]
        : [made, tag, HTML_NAMESPACE, null, tag.toUpperCase()],
    ];
    const copyInto = (parent: number, sources: number[]): void => {
      for (const child of sources) {
        const copied = copyOf(child, deep);
        place(parent, copied[0]![0], null);
        entries.push(...copied);
      }
    };
    if (content !== null) {
      const madeContent = nodeOf(made).content!;
      entries.push([madeContent, "#fragment", null]);
      if (deep) copyInto(madeContent, nodeOf(content).children);
    }
    if (deep) copyInto(made, children);
    return entries;
  };

  const orphans = (): number[] => {
    const contents = new Set([...nodes.values()].map((node) => node.content));
    return [...nodes]
      .filter(
        ([id, node]) =>
          node.parent === null &&
          !held.has(id) &&
          id !== ROOT &&
          id !== DOCUMENT &&
          !contents.has(id),
      )
      .map(([id]) => id);
  };

  const spies = {
    cloneNode: vi.fn<Native["cloneNode"]>(copyOf),
    linkedNode: vi.fn<Native["linkedNode"]>((id, link, elementOnly) =>
      linked(id, link, elementOnly === true),
    ),
    childNodeIds: vi.fn<Native["childNodeIds"]>((id, elementOnly) =>
      nodeOf(id).children.filter((child) => elementOnly !== true || isElement(child)),
    ),
    removeNode: vi.fn<Native["removeNode"]>((id) => {
      if (!nodes.has(id)) return [[], []];
      detach(id);
      held.add(id);
      const ids = subtree(id);
      return [dropListeners(ids), ids];
    }),
    holdNode: vi.fn<Native["holdNode"]>((id) => {
      nodeOf(id);
      held.add(id);
    }),
    releaseNode: vi.fn<Native["releaseNode"]>((id) => void held.delete(id)),
    insertBefore: vi.fn<Native["insertBefore"]>((parentId, nodeId, refId) =>
      place(parentId, nodeId, refId),
    ),
    removeChild: vi.fn<Native["removeChild"]>((parentId, childId) => {
      nodeOf(parentId);
      if (!nodes.has(childId)) return;
      if (nodeOf(childId).parent !== parentId) {
        throw new DOMException("not a child", "NotFoundError");
      }
      detach(childId);
    }),
  };

  const natives: Partial<Native> = {
    rootNodeId: () => ROOT,
    documentNodeId: () => DOCUMENT,
    createNode: create,
    createElement: (name) => [create(name), name],
    createElementNS: (namespace, name) => [create(name), name, namespace, null, name],
    templateContent: (id) => nodeOf(id).content,
    appendChild: (parentId, nodeId) => place(parentId, nodeId, null),
    removeChild: spies.removeChild,
    replaceChild(parentId, nodeId, childId) {
      if (nodeOf(childId).parent !== parentId)
        throw new DOMException("not a child", "NotFoundError");
      let ref = nextOf(childId);
      if (ref === nodeId) ref = nextOf(nodeId);
      detach(childId);
      return place(parentId, nodeId, ref);
    },
    rootOf(id) {
      let at = id;
      while (nodeOf(at).parent !== null) at = nodeOf(at).parent!;
      return at;
    },
    isConnected: (id) => natives.rootOf!(id) === DOCUMENT,
    closest(id, tag) {
      for (let at: number | null = id; at !== null; at = nodeOf(at).parent) {
        if (nodeOf(at).tag === tag) return at;
      }
      return null;
    },
    querySelector: (id, tag) => matching(id, tag)[0] ?? null,
    querySelectorAll: (id, tag) => matching(id, tag),
    addEventListener(id, _event, callback) {
      const callbackId = nextCallbackId++;
      nodeOf(id as number).listeners.push(callbackId);
      callbacks.set(callbackId, callback);
      return callbackId;
    },
    removeEventListener(id, _event, callbackId) {
      const listeners = nodeOf(id as number).listeners;
      const at = listeners.indexOf(callbackId);
      if (at !== -1) listeners.splice(at, 1);
      callbacks.delete(callbackId);
      return at !== -1;
    },
    attributeNames: (id) => (nodeOf(id), []),
    inputValue: (id) => (nodeOf(id), ""),
    setInputValue: (id) => void nodeOf(id),
    focusedNode: () => null,
    layoutOf: () => null,
    scrollOf: () => null,
    getAttribute: (id, key) =>
      (key === "value" ? nodeOf(id).data : (nodeOf(id).attrs.get(key) ?? null)) as never,
    setAttribute(id, key, value) {
      const node = nodeOf(id);
      node.attrs.set(key, value);
      if (key === "value") node.data = String(value);
    },
    removeAttribute: (id) => void nodeOf(id),
    setStyle: (id) => void nodeOf(id),
    removeStyle: (id) => void nodeOf(id),
    splitText(id) {
      const tail = create("#text");
      const node = nodeOf(id);
      if (node.parent !== null) place(node.parent, tail, nextOf(id));
      return tail;
    },
    cloneNode: spies.cloneNode,
    linkedNode: spies.linkedNode,
    childNodeIds: spies.childNodeIds,
    removeNode: spies.removeNode,
    holdNode: spies.holdNode,
    releaseNode: spies.releaseNode,
    insertBefore: spies.insertBefore,
  };
  globalThis.__inca_native__ = nativeStub({ ...natives, ...extra });

  return {
    DOCUMENT,
    ROOT,
    nodes,
    held,
    callbacks,
    natives,
    spies,
    create,
    idsOf: (tag) => [...nodes].filter(([, node]) => node.tag === tag).map(([id]) => id),
    orphans,
    frame() {
      const freed: number[] = [];
      const listeners: number[] = [];
      for (const id of orphans()) {
        if (!nodes.has(id)) continue;
        for (const gone of subtree(id)) {
          listeners.push(...nodeOf(gone).listeners);
          for (const callbackId of nodeOf(gone).listeners) callbacks.delete(callbackId);
          nodes.delete(gone);
          held.delete(gone);
          freed.push(gone);
        }
      }
      if (freed.length > 0) globalThis.__inca_released__(freed, listeners);
    },
  };
}
