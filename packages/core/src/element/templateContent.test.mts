// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import { document } from "../document.mts";
import { createElementNode } from "./handle.mts";
import { isDestroyed, markDestroyed } from "./lifecycle.mts";
import type { IncaTemplateElement } from "./types.mts";

let nextId = 1;
/** The ids of the templates in the fake host. Each content takes the id after its template. */
let templates = new Set<number>();

const host = {
  createNode: vi.fn<(tag: string) => number>((tag) => {
    const id = nextId;
    nextId += tag === "template" ? 2 : 1;
    if (tag === "template") templates.add(id);
    return id;
  }),
  templateContent: vi.fn<(nodeId: number) => number | null>((id) =>
    templates.has(id) ? id + 1 : null,
  ),
  appendChild: vi.fn<() => number>(() => 0),
  setAttribute: vi.fn<() => void>(),
  setStyle: vi.fn<() => void>(),
  holdNode: vi.fn<(nodeId: number) => void>(),
  releaseNode: vi.fn<(nodeId: number) => void>(),
};

beforeEach(() => {
  vi.clearAllMocks();
  host.templateContent.mockImplementation((id) => (templates.has(id) ? id + 1 : null));
  nextId = 1;
  templates = new Set();
  globalThis.__inca_native__ = nativeStub(host as never);
});

const template = () => createElementNode("template") as IncaTemplateElement;

describe("template.content", () => {
  it("is a fragment with the id the host holds as the content", () => {
    const owner = template();

    expect([owner.content.kind, owner.content.__node_id]).toEqual([
      "fragment",
      owner.__node_id + 1,
    ]);
  });

  it("is the same fragment on each read", () => {
    const owner = template();

    expect(owner.content).toBe(owner.content);
  });

  it("is missing on a div", () => {
    expect("content" in createElementNode("div")).toBe(false);
  });

  it("reads document as the owner document of the content and of a node in it", () => {
    const owner = template();
    const inside = createElementNode("div");
    owner.content.appendChild(inside);

    expect([owner.content.ownerDocument, inside.ownerDocument]).toEqual([document, document]);
  });
});

describe("the link between a template and its content", () => {
  it("holds the template in the host", () => {
    const owner = template();

    expect(host.holdNode).toHaveBeenCalledWith(owner.__node_id);
  });

  it("marks the template destroyed when only the content is marked", () => {
    const owner = template();

    markDestroyed(owner.content);

    expect(isDestroyed(owner)).toBe(true);
  });

  it("reads the same content object after the host freed the template", () => {
    const owner = template();
    const content = owner.content;
    host.templateContent.mockImplementation(() => {
      throw new TypeError("freed");
    });
    markDestroyed(owner);

    expect(owner.content).toBe(content);
  });
});
