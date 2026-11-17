// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { RendererOptions } from "@vue/runtime-core";

import type {
  Comment,
  RendererCore,
  Element,
  Node,
  Text,
  TagName,
  HandleNode,
} from "../rendererCore.mts";

/** {@link createNodeOps}'s return shape. */
export type NodeOps = Omit<RendererOptions<Node, Element>, "patchProp">;

/**
 * `@vue/runtime-core`'s {@link RendererOptions}`<Node, Element>`,
 * minus `patchProp`. This is the host-node lifecycle half of `incajs/vue`'s custom
 * renderer.
 * @param core - the incajs bindings to drive the native tree through
 */
export function createNodeOps(core: RendererCore): NodeOps {
  return {
    /**
     * Allocates a new element node for `tag`. Vue's other `createElement`
     * parameters (namespace, `is`, initial props) are ignored.
     * @param tag - the node's element kind
     * @returns the new, parentless, childless element
     */
    createElement: (tag: TagName): Element => core.createElementNode(tag),

    /**
     * Creates a text leaf.
     * @param text - the initial text content
     * @returns the new text leaf
     */
    createText: (text: string): Text => core.createTextNode(text),

    /**
     * Creates a comment, such as a `v-if`/`v-for` placeholder.
     * @param text - the comment's text
     * @returns the new comment
     */
    createComment: (text: string): Comment => core.createCommentNode(text),

    setText: (node: Text, text: string): void => core.setText(node, text),

    /**
     * Replaces all of `el`'s children with at most a single text child. Vue
     * takes this path for an element whose only dynamic content is its own text.
     * @param el - the container whose children are replaced
     * @param text - the new text content
     */
    setElementText: (el: Element, text: string): void => core.setElementText(el, text),

    /**
     * Attaches `child` to `parent`, before `anchor` or at the end. Vue moves a
     * node during a keyed-list reorder through this call.
     * @param child - the node being attached
     * @param parent - the new container
     * @param anchor - the sibling to insert before, or omitted/`null` to append at the end
     */
    insert: (child: HandleNode, parent: Element, anchor?: Node | null): void =>
      core.insert(child, parent, anchor),

    /**
     * Marks `el` with the empty attribute `id`. A scoped style rule selects the
     * elements of its component through it.
     * @param el - the element of a component with scoped styles
     * @param id - the attribute name Vue generated for the component
     */
    setScopeId: (el: Element, id: string): void => core.setAttribute(el.__node_id, id, ""),

    /**
     * Removes `child` with its subtree. Vue unmounts a subtree by calling this
     * on its root alone.
     * @param child - the node being removed
     */
    remove: (child: Node): void => core.remove(child),

    parentNode: core.parentNode,
    nextSibling: core.nextSibling,
  };
}

type Creators = Pick<NodeOps, "createElement" | "createText">;

/**
 * The creators of `ops` that hold the node they create and release the hold
 * once the returned handle is collected.
 * @param core - the incajs bindings the holds go through
 * @param ops - the creators to wrap
 * @returns held versions of `createElement` and `createText`
 */
export function createHeldNodeOps(core: RendererCore, ops: Creators): Creators {
  const hold = <N extends Node>(node: N): N => {
    core.holdNode(node.__node_id);
    core.releaseWhenCollected(node);
    return node;
  };
  // Read now: the caller may assign the result over `ops`.
  const { createElement, createText } = ops;
  return {
    createElement: (tag) => hold(createElement(tag)),
    createText: (text) => hold(createText(text)),
  };
}
