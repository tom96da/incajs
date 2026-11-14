// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { createIdTable } from "./idTable.mts";

describe("createIdTable", () => {
  it("reads what set wrote, in two chunks", () => {
    const table = createIdTable<string>();

    table.set(5, "a");
    table.set(1030, "b");

    expect([table.get(5), table.get(1030)]).toEqual(["a", "b"]);
  });

  it("keeps the other entry readable when one of two entries of a chunk is deleted", () => {
    const table = createIdTable<string>();
    table.set(1, "a");
    table.set(2, "b");

    table.delete(1);

    expect([table.get(1), table.get(2)]).toEqual([undefined, "b"]);
  });

  it("rebuilds a chunk that dropped its last entry", () => {
    const table = createIdTable<string>();
    table.set(1, "a");
    table.set(2, "b");
    table.delete(1);
    table.delete(2);

    table.set(3, "c");

    expect([table.get(1), table.get(2), table.get(3)]).toEqual([undefined, undefined, "c"]);
  });

  it("reads back a set far past the outer length", () => {
    const table = createIdTable<string>();

    table.set(5_000_000, "far");

    expect([table.get(5_000_000), table.get(4)]).toEqual(["far", undefined]);
  });

  it("ignores the delete of an id that holds nothing", () => {
    const table = createIdTable<string>();
    table.set(1, "a");

    table.delete(2);
    table.delete(4096);

    expect(table.get(1)).toBe("a");
  });
});
