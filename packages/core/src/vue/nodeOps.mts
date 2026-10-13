// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { RendererOptions } from "@vue/runtime-core";

import {
  createCommentNode,
  createElementNode,
  createTextNode,
  insert,
  nextSibling,
  parentNode,
  remove,
  setElementText,
  setText,
} from "../element/shadowTree.mts";
import type { IncaElement, IncaNode, IncaText } from "../element/types.mts";
import type { IncaCore, TagName } from "../rendererCore.mts";

/**
 * Marks `handle` so that Vue's `reactive()` and `ref()` keep the object itself.
 * A template ref then holds the same object the application receives from
 * `activeElement()`.
 * @param handle - the node handle to mark
 * @returns `handle`
 */
export function skipReactive<T extends object>(handle: T): T {
  Object.defineProperty(handle, "__v_skip", { value: true });
  return handle;
}

/** {@link createNodeOps}'s return shape. */
export type NodeOps = Omit<RendererOptions<IncaNode, IncaElement>, "patchProp">;

/**
 * `@vue/runtime-core`'s {@link RendererOptions}`<IncaNode, IncaElement>`,
 * minus `patchProp`. This is the host-node lifecycle half of `incajs/vue`'s custom
 * renderer.
 * @param core - the incajs bindings to drive the native tree through
 */
export function createNodeOps(core: IncaCore): NodeOps {
  return {
    /**
     * Allocates a new element node for `tag`. Vue's other `createElement`
     * parameters (namespace, `is`, initial props) are ignored.
     * @param tag - the node's element kind
     * @returns the new, parentless, childless element
     */
    createElement: (tag: TagName): IncaElement => skipReactive(createElementNode(core, tag)),

    /**
     * Creates a text leaf.
     * @param text - the initial text content
     * @returns the new text leaf
     */
    createText: (text: string): IncaText => skipReactive(createTextNode(core, text)),

    /**
     * Creates the hidden container that stands in for a Vue comment node
     * (e.g. a `v-if`/`v-for` placeholder). The comment's text is discarded.
     * @param _text - the comment's text
     * @returns the new hidden element
     */
    createComment: (_text: string): IncaElement => skipReactive(createCommentNode(core)),

    setText: (node: IncaText, text: string): void => setText(core, node, text),

    /**
     * Replaces all of `el`'s children with at most a single text child. Vue
     * takes this path for an element whose only dynamic content is its own text.
     * @param el - the container whose children are replaced
     * @param text - the new text content
     */
    setElementText: (el: IncaElement, text: string): void => setElementText(core, el, text),

    /**
     * Attaches `child` to `parent`, before `anchor` or at the end. Vue moves a
     * node during a keyed-list reorder through this call.
     * @param child - the node being attached
     * @param parent - the new container
     * @param anchor - the sibling to insert before, or omitted/`null` to append at the end
     */
    insert: (child: IncaNode, parent: IncaElement, anchor?: IncaNode | null): void =>
      insert(core, child, parent, anchor),

    /**
     * Removes `child` with its subtree. Vue unmounts a subtree by calling this
     * on its root alone.
     * @param child - the node being removed
     */
    remove: (child: IncaNode): void => remove(core, child),

    parentNode,
    nextSibling,
  };
}
