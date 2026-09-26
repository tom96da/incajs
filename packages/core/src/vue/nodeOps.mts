// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { RendererOptions } from "@vue/runtime-core";

import type { IncaCore } from "../rendererCore.mts";
import type { NodeId, TagName } from "../types.mts";

/**
 * A container host node: either a real element or the hidden stand-in
 * {@link NodeOps.createComment} uses. Also `@vue/runtime-core`'s
 * `HostElement` — the only host node type that can act as a parent.
 *
 * `inca`'s native tree has no parent pointers and no way to list a
 * node's children, so `parent`/`children` are maintained here as a
 * JS-side shadow of the tree, kept in sync by {@link NodeOps.insert}/
 * {@link NodeOps.remove}.
 */
export interface IncaElement {
  /**
   * The underlying native node's id — the only part of this object the native
   * tree itself knows about.
   */
  readonly id: NodeId;
  /**
   * `"comment"` for the hidden stand-in {@link NodeOps.createComment}
   * produces; `"element"` for every other container. Purely
   * informational — both render identically once created.
   */
  readonly kind: "element" | "comment";
  /** The current parent, or `null` if this node isn't attached to the tree. */
  parent: IncaElement | null;
  /** This node's children, in render order. */
  children: IncaNode[];
  /** Focuses this node next frame. */
  focus(): void;
  /** Unfocuses whatever's focused, next frame. */
  blur(): void;
}

/** A text leaf host node — `@vue/runtime-core`'s `HostNode` for text. */
export interface IncaText {
  /** The underlying native node's id. */
  readonly id: NodeId;
  /** Discriminates this node within the {@link IncaNode} union. */
  readonly kind: "text";
  /** The current parent, or `null` if this node isn't attached to the tree. */
  parent: IncaElement | null;
  /** The text content, mirroring what's been pushed to the native node via {@link setAttribute}. */
  text: string;
}

/** Any host node {@link NodeOps} can produce: an element, comment, or text leaf. */
export type IncaNode = IncaElement | IncaText;

/** {@link createNodeOps}'s return shape. */
export type NodeOps = Omit<RendererOptions<IncaNode, IncaElement>, "patchProp">;

/**
 * `@vue/runtime-core`'s {@link RendererOptions}`<IncaNode, IncaElement>`,
 * minus `patchProp` (see `./patchProp.mts`) — the host-node lifecycle half
 * of `incajs/vue`'s custom renderer, built entirely on the injected
 * {@link IncaCore}, never on the native bridge directly.
 * @param core - the incajs bindings to drive the native tree through
 */
export function createNodeOps(core: IncaCore): NodeOps {
  function createTextNode(text: string): IncaText {
    const id = core.createNode("text");
    core.setAttribute(id, "value", text);
    return { id, kind: "text", parent: null, text };
  }

  // Drops `child` from its parent in this module's shadow of the native tree.
  function unlink(child: IncaNode): void {
    const parent = child.parent;
    if (!parent) return;

    const index = parent.children.indexOf(child);
    if (index !== -1) parent.children.splice(index, 1);
    child.parent = null;
  }

  // Detaches `child` in the native tree as well. `destroyNode` detaches on
  // its own, so the removal path uses `unlink` instead.
  function detach(child: IncaNode): void {
    if (child.parent) core.removeChild(child.parent.id, child.id);
    unlink(child);
  }

  // Shared by createElement/createComment.
  function focusMethods(id: NodeId): Pick<IncaElement, "focus" | "blur"> {
    return {
      focus: () => core.focus(id),
      blur: () => core.blur(),
    };
  }

  return {
    /**
     * Allocates a new element node for `tag`. Vue's other `createElement`
     * parameters (namespace, `is`, initial props) don't apply here — GPUI
     * has no SVG/MathML/custom-element concept — and are ignored.
     * @param tag - the node's element kind
     * @returns the new, parentless, childless element
     */
    createElement(tag: TagName): IncaElement {
      const id = core.createNode(tag);
      return { id, kind: "element", parent: null, children: [], ...focusMethods(id) };
    },

    /**
     * Creates a text leaf.
     * @param text - the initial text content
     * @returns the new text leaf
     */
    createText: createTextNode,

    // inca has no comment concept — a hidden, childless container stands
    // in for one. The comment's own text carries no rendered meaning here and
    // is discarded, same as the hidden node itself.
    /**
     * Creates the hidden container that stands in for a Vue comment node
     * (e.g. a `v-if`/`v-for` placeholder).
     * @param _text - the comment's text; accepted for interface compatibility, but discarded since nothing renders it
     * @returns the new hidden element
     */
    createComment(_text: string): IncaElement {
      const id = core.createNode("div");
      core.setStyle(id, "display", "none");
      return { id, kind: "comment", parent: null, children: [], ...focusMethods(id) };
    },

    /**
     * Updates a text leaf's content in place.
     * @param node - the text leaf to update
     * @param text - the new text content
     */
    setText(node: IncaText, text: string): void {
      core.setAttribute(node.id, "value", text);
      node.text = text;
    },

    /**
     * Replaces all of `el`'s children with, at most, a single text child
     * holding `text` (or none, for an empty string) — the fast path Vue takes
     * for an element whose only dynamic content is its own text.
     * @param el - the container whose children are replaced
     * @param text - the new text content
     */
    setElementText(el: IncaElement, text: string): void {
      for (const child of el.children.splice(0)) {
        core.destroyNode(child.id);
        child.parent = null;
      }
      if (!text) return;

      const textNode = createTextNode(text);
      core.appendChild(el.id, textNode.id);
      textNode.parent = el;
      el.children.push(textNode);
    },

    /**
     * Attaches `child` to `parent`, before `anchor` (or at the end, if
     * `anchor` is omitted/`null`). If `child` is already attached elsewhere,
     * it's detached first — this is how Vue moves a node during a keyed-list
     * reorder, so the node is never disposed or recreated for a move.
     * @param child - the node being attached
     * @param parent - the new container
     * @param anchor - the sibling to insert before, or omitted/`null` to append at the end
     */
    insert(child: IncaNode, parent: IncaElement, anchor?: IncaNode | null): void {
      detach(child);

      const anchorIndex = anchor ? parent.children.indexOf(anchor) : -1;
      core.insertBefore(parent.id, child.id, anchor && anchorIndex !== -1 ? anchor.id : null);
      if (anchorIndex === -1) {
        parent.children.push(child);
      } else {
        parent.children.splice(anchorIndex, 0, child);
      }
      child.parent = parent;
    },

    /**
     * Removes `child` for good: the native node, its whole subtree, and every
     * event listener registered in it.
     *
     * Vue unmounts a subtree by calling this on its root alone, so a node's
     * descendants are freed here or nowhere.
     * @param child - the node being removed
     */
    remove(child: IncaNode): void {
      unlink(child);
      core.destroyNode(child.id);
    },

    /**
     * Looks up the current parent.
     * @param node - the node to query
     * @returns the current parent, or `null` if it isn't attached
     */
    parentNode(node: IncaNode): IncaElement | null {
      return node.parent;
    },

    /**
     * Looks up the next sibling.
     * @param node - the node to query
     * @returns the sibling immediately after `node` in its parent's children, or `null` if there is none (or `node` isn't attached)
     */
    nextSibling(node: IncaNode): IncaNode | null {
      const parent = node.parent;
      if (!parent) return null;

      const index = parent.children.indexOf(node);
      return parent.children[index + 1] ?? null;
    },
  };
}
