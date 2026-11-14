// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// A sparse table from a small integer id to a value, kept as chunks of 1024 slots.

const CHUNK_BITS = 10;
const CHUNK_SIZE = 1 << CHUNK_BITS;

/** The table {@link createIdTable} builds. */
export interface IdTable<T> {
  /**
   * Reads the value of an id.
   * @param id - the id to read
   * @returns the value, or `undefined` when the id holds none
   */
  get(id: number): T | undefined;
  /**
   * Stores the value of an id.
   * @param id - the id to write
   * @param value - the value to keep
   */
  set(id: number, value: T): void;
  /**
   * Clears the value of an id. An id with no value stays as it is.
   * @param id - the id to clear
   */
  delete(id: number): void;
}

/**
 * Creates an empty table. An outer array holds one slot for each 1024 ids, and a slot holds a
 * built-filled chunk or `undefined`. A chunk that loses its last value is dropped.
 * @returns the table
 */
export function createIdTable<T>(): IdTable<T> {
  const chunks: ((T | undefined)[] | undefined)[] = [];
  const counts: number[] = [];
  return {
    get: (id) => chunks[id >>> CHUNK_BITS]?.[id & (CHUNK_SIZE - 1)],
    set(id, value) {
      const at = id >>> CHUNK_BITS;
      while (chunks.length <= at) {
        chunks.push(undefined);
        counts.push(0);
      }
      let chunk = chunks[at];
      if (chunk === undefined) {
        chunk = chunks[at] = Array.from<T | undefined>({ length: CHUNK_SIZE });
      }
      const slot = id & (CHUNK_SIZE - 1);
      if (chunk[slot] === undefined) counts[at]!++;
      chunk[slot] = value;
    },
    delete(id) {
      const at = id >>> CHUNK_BITS;
      const chunk = chunks[at];
      const slot = id & (CHUNK_SIZE - 1);
      if (chunk?.[slot] === undefined) return;
      chunk[slot] = undefined;
      if (--counts[at]! === 0) chunks[at] = undefined;
    },
  };
}
