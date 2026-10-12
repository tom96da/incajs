// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { h, reactive } from "@vue/runtime-core";
import { describe, expect, it, vi } from "vitest";

import { createIncaApp } from "./createApp.mts";
import type { IncaElement } from "../element/types.mts";

vi.mock("../rendererCore.mts", () => {
  let nextId = 2;
  return {
    rootNodeId: () => 1,
    createNode: () => nextId++,
    insertBefore: () => {},
    createEventTarget: () => ({}),
  };
});

describe("createIncaApp", () => {
  it("keeps the same root handle inside reactive()", () => {
    const app = createIncaApp({ render: () => h("div") });
    app.mount();
    const root = (app as unknown as { _container: IncaElement })._container;

    expect(root.id).toBe(1);
    expect(reactive(root)).toBe(root);
  });
});
