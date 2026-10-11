// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

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
