// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * Reads a `tabindex` attribute value as the element's tab index. A value that
 * is missing, not a string or number, has no leading integer, or falls outside
 * the 32-bit signed range yields the default.
 * @param value - the stored `tabindex` attribute, or `null` if unset
 * @param kind - the element kind; `"button"` defaults to `0`, any other kind to `-1`
 * @returns the parsed tab index, or the default
 */
export function tabIndexOf(value: string | number | boolean | null, kind: string): number {
  const fallback = kind === "button" ? 0 : -1;
  if (typeof value !== "string" && typeof value !== "number") return fallback;

  const match = /^[\t\n\f\r ]*([+-]?\d+)/.exec(String(value));
  if (!match) return fallback;

  const n = Number(match[1]) || 0;
  return n >= -2147483648 && n <= 2147483647 ? n : fallback;
}
