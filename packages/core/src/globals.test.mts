// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expectTypeOf, it } from "vitest";

import type * as incajs from "./index.mts";

// These tests pin the global names of `globals.d.mts`. The file imports no type by its bare name,
// so each bare name is the global one and `incajs.X` is the exported one. A function under test
// is never called, so the compiler checks it and `pnpm typecheck` fails on a mismatch.
describe("the global types", () => {
  it("give document the type Document", () => {
    const create = (): HTMLElement => document.createElement("div");
    expectTypeOf(create).returns.toEqualTypeOf<HTMLElement>();
    expectTypeOf<typeof document>().toEqualTypeOf<Document>();
  });

  it("give window the type Window", () => {
    expectTypeOf<typeof window>().toEqualTypeOf<Window>();
  });

  it("make the exported and the global HTMLInputElement one type", () => {
    expectTypeOf<HTMLInputElement>().toEqualTypeOf<incajs.HTMLInputElement>();
  });

  it("declare the node interfaces and the mixins under their exported types", () => {
    expectTypeOf<Node>().toEqualTypeOf<incajs.Node>();
    expectTypeOf<ParentNode>().toEqualTypeOf<incajs.ParentNode>();
    expectTypeOf<ChildNode>().toEqualTypeOf<incajs.ChildNode>();
    expectTypeOf<NonDocumentTypeChildNode>().toEqualTypeOf<incajs.NonDocumentTypeChildNode>();
    expectTypeOf<CharacterData>().toEqualTypeOf<incajs.CharacterData>();
  });

  it("name the legacy HTML document the document type", () => {
    expectTypeOf<HTMLDocument>().toEqualTypeOf<Document>();
  });

  it("narrow a node to an element through instanceof", () => {
    const narrow = (node: Node) => {
      if (node instanceof Element) return node.tagName;
      return undefined;
    };
    expectTypeOf(narrow).returns.toEqualTypeOf<string | undefined>();
  });

  it("keep the constraint of the generic lists", () => {
    expectTypeOf<NodeListOf<Element>>().toHaveProperty("length");
    expectTypeOf<HTMLCollectionOf<HTMLElement>>().toHaveProperty("namedItem");
    // @ts-expect-error a list holds nodes
    expectTypeOf<NodeListOf<string>>().toBeObject();
    // @ts-expect-error a collection holds elements
    expectTypeOf<HTMLCollectionOf<string>>().toBeObject();
  });
});

describe("the exported names", () => {
  it("export no IncaElement type", () => {
    // @ts-expect-error the old name is gone
    expectTypeOf<incajs.IncaElement>().toBeObject();
  });
});
