// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The Document and ShadowRoot interface objects, installed as globals on load.

/**
 * The interface of `document`; `document instanceof Document` holds.
 * @throws TypeError always, since a document cannot be constructed
 */
export function Document(): never {
  throw new TypeError("document cannot be constructed. use the document export of incajs");
}

/**
 * The interface of a shadow root. No node is one.
 * @throws TypeError always, since a shadow root cannot be constructed
 */
export function ShadowRoot(): never {
  throw new TypeError("a shadow root cannot be constructed. the host has no shadow roots");
}

Object.defineProperty(globalThis, "Document", {
  value: Document,
  writable: true,
  configurable: true,
  enumerable: false,
});
Object.defineProperty(globalThis, "ShadowRoot", {
  value: ShadowRoot,
  writable: true,
  configurable: true,
  enumerable: false,
});
