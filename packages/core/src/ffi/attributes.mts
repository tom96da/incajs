// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { native } from "./native.mts";
import type { CSSStyleDeclaration, DOMTokenList } from "../element/types.mts";
import type { AttributeValue, NodeId, StyleProps } from "../types.mts";

/**
 * Sets an attribute/prop on `nodeId`. The key `style` applies the text as the inline style
 * declarations. See {@link setStyle} for layout/paint properties.
 * @param nodeId - the node to update
 * @param key - the attribute name
 * @param value - the attribute value
 * @throws {DOMException} named `InvalidCharacterError` when `key` is not a valid attribute name
 */
export function setAttribute(nodeId: NodeId, key: string, value: AttributeValue): void {
  native().setAttribute(nodeId, key, value);
}

/**
 * Returns the key that the attribute `name` has on `nodeId`: the name in lower case for an HTML
 * element, and unchanged for any other element.
 * @param nodeId - the node to read
 * @param name - the attribute name
 * @returns the key
 * @throws {TypeError} when `nodeId` names no node
 */
export function attributeKey(nodeId: NodeId, name: string): string {
  return native().attributeKey(nodeId, name);
}

/**
 * Returns the attribute/prop stored on `nodeId` under `key`, or `null` when unset.
 * @param nodeId - the node to read
 * @param key - the attribute name
 * @returns the stored value, or `null`
 * @throws {TypeError} when `nodeId` names no node
 */
export function getAttribute(nodeId: NodeId, key: string): AttributeValue | null {
  return native().getAttribute(nodeId, key);
}

/**
 * Returns the token list of the `class` attribute of `nodeId`. A list of a freed node reads empty.
 * @param nodeId - the node
 * @returns the live list
 */
export function classList(nodeId: NodeId): DOMTokenList {
  return native().classList(nodeId);
}

/**
 * Returns the names of the attributes stored on `nodeId` in the order they were set, in the lower
 * case the host keeps them in.
 * @param nodeId - the node to read
 * @returns the attribute names
 * @throws {TypeError} when `nodeId` names no node
 */
export function attributeNames(nodeId: NodeId): string[] {
  return native().attributeNames(nodeId);
}

/**
 * Reads the `data-*` attribute that the `dataset` key `key` names.
 * @param nodeId - the node to read
 * @param key - the camelCase dataset key
 * @returns the value as text, or `undefined` when the attribute is absent or the key names none
 */
export function datasetGet(nodeId: NodeId, key: string): string | undefined {
  return native().datasetGet(nodeId, key);
}

/**
 * Writes the `data-*` attribute that the `dataset` key `key` names. A freed node ignores the
 * write.
 * @param nodeId - the node to update
 * @param key - the camelCase dataset key
 * @param value - the attribute text
 * @throws {DOMException} named `SyntaxError` for a key with a dash before a lower case letter, and
 * named `InvalidCharacterError` for a key that makes an invalid attribute name
 */
export function datasetSet(nodeId: NodeId, key: string, value: string): void {
  native().datasetSet(nodeId, key, value);
}

/**
 * Removes the `data-*` attribute that the `dataset` key `key` names.
 * @param nodeId - the node to update
 * @param key - the camelCase dataset key
 * @returns `true` in every case, also for an absent attribute
 */
export function datasetDelete(nodeId: NodeId, key: string): boolean {
  return native().datasetDelete(nodeId, key);
}

/**
 * Lists the `dataset` keys of `nodeId` in attribute order.
 * @param nodeId - the node to read
 * @returns the keys, empty for a freed node
 */
export function datasetKeys(nodeId: NodeId): string[] {
  return native().datasetKeys(nodeId);
}

/**
 * Creates the target object of a `dataset` proxy. Its prototype is the `DOMStringMap` prototype.
 * @returns the target
 */
export function datasetTarget(): object {
  return native().datasetTarget();
}

/**
 * Removes an attribute/prop from `nodeId`. Removing an attribute
 * that isn't set does nothing.
 * @param nodeId - the node to update
 * @param key - the attribute name
 */
export function removeAttribute(nodeId: NodeId, key: string): void {
  native().removeAttribute(nodeId, key);
}

/**
 * Sets a style property on `nodeId`. A key from {@link StyleProps} gets
 * compile-time checking on its value's shape. Any other string key, such as a
 * camelCase or kebab-case spelling or a property the types do not list,
 * forwards as a raw {@link AttributeValue}.
 *
 * A key takes kebab-case (`flex-direction`) or camelCase (`flexDirection`).
 * Both spellings share one slot per property, and the value set most recently
 * applies.
 * @param nodeId - the node to update
 * @param key - the style property name
 * @param value - the style property value
 */
export function setStyle<K extends keyof StyleProps>(
  nodeId: NodeId,
  key: K,
  value: NonNullable<StyleProps[K]>,
): void;
export function setStyle(nodeId: NodeId, key: string, value: AttributeValue): void;
export function setStyle(nodeId: NodeId, key: string, value: AttributeValue): void {
  native().setStyle(nodeId, key, value);
}

/**
 * Removes a style property from `nodeId`, so it renders as if never set.
 * Removing a property that isn't set does nothing. The key takes any spelling
 * or CSS name {@link setStyle} accepts, and removes the property whichever
 * one set it.
 * @param nodeId - the node to update
 * @param key - the style property name
 */
export function removeStyle(nodeId: NodeId, key: string): void {
  native().removeStyle(nodeId, key);
}

/**
 * Returns the style declaration block of `nodeId`. Each call returns a new object over the node's
 * inline style. The block of a freed node reads empty and ignores writes.
 * @param nodeId - the node whose inline style the block reads and writes
 * @returns the block
 */
export function styleOf(nodeId: NodeId): CSSStyleDeclaration {
  return native().styleOf(nodeId);
}

/**
 * Returns the text of the `input` node `nodeId`.
 * @param nodeId - the `input` node to read
 * @returns the text of the input
 * @throws TypeError for a node that is not an input
 */
export function inputValue(nodeId: NodeId): string {
  return native().inputValue(nodeId);
}

/**
 * Replaces the text of the `input` node `nodeId`. Line breaks are dropped, and a changed text
 * puts the caret at its end.
 * @param nodeId - the `input` node to update
 * @param value - the new text
 * @throws TypeError for a node that is not an input
 */
export function setInputValue(nodeId: NodeId, value: string): void {
  native().setInputValue(nodeId, value);
}
