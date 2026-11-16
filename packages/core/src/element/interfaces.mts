// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// Defines each member of a node once, on the prototype of the interface that has it.

import { eventTargetMembers } from "../eventTarget.mts";
import { attributeMembers, nameMembers } from "./attributes.mts";
import { classMembers } from "./classList.mts";
import { datasetMembers } from "./dataset.mts";
import { documentMembers } from "./documentMembers.mts";
import { nonDocumentTypeChildNodeMembers, parentNodeMembers } from "./elementTree.mts";
import { handlerMembers } from "./eventHandlers.mts";
import { layoutMembers, offsetMembers } from "./geometry.mts";
import { htmlMembers } from "./html.mts";
import { styleMembers } from "./inlineStyle.mts";
import { childNodeMutationMembers, parentNodeMutationMembers } from "./insertionMembers.mts";
import { clickMembers, focusMembers, pointerCaptureMembers } from "./interaction.mts";
import {
  DOCUMENT_RULE,
  HTML_OR_SVG_RULE,
  TARGET_RULE,
  prototypes,
  receiverOf,
} from "./lifecycle.mts";
import { nodeMutationMembers } from "./nodeMutation.mts";
import { nodeRelationMembers } from "./nodeRelations.mts";
import { nodeTreeMembers } from "./nodeTree.mts";
import { scrollMembers } from "./scroll.mts";
import { queryMembers, selectorMembers } from "./selectors.mts";
import { characterDataMembers, textMembers } from "./text.mts";
import type { HandleBuilders } from "./lifecycle.mts";

/**
 * Defines `members` on `proto`. A method is writable, enumerable and configurable, and an accessor
 * is enumerable and configurable, so a second evaluation of core defines them again.
 * @param proto - the prototype
 * @param members - an object literal that holds the members
 */
export function defineMembers(proto: object, members: object): void {
  Object.defineProperties(proto, Object.getOwnPropertyDescriptors(members));
}

/**
 * Builds the handler properties of `window`, which it holds as own properties.
 * @returns the accessors
 */
export function windowHandlerMembers(): object {
  return handlerMembers((self) => receiverOf(self, TARGET_RULE));
}

/**
 * Defines the data property `kind` of a prototype: non-writable, non-enumerable, configurable.
 * @param proto - the prototype
 * @param kind - the kind of its handles
 */
function defineKind(proto: object, kind: string): void {
  Object.defineProperty(proto, "kind", { value: kind, configurable: true });
}

/**
 * Defines the members that insert nodes and lists them in the `Symbol.unscopables` of `proto`.
 * @param proto - the prototype
 * @param sets - the member objects, built for this prototype
 */
function defineInsertionMembers(proto: object, ...sets: object[]): void {
  const names = Object.create(null) as Record<string, true>;
  for (const set of sets) {
    defineMembers(proto, set);
    for (const name of Object.keys(set)) names[name] = true;
  }
  Object.defineProperty(proto, Symbol.unscopables, { value: names, configurable: true });
}

/**
 * Defines the members of the node interfaces on the host prototypes. Each mixin member is built
 * again for each interface that includes it, so each interface has its own functions.
 * @param builders - the functions that build and find handles
 */
export function installInterfaces(builders: HandleBuilders): void {
  const all = prototypes();
  const shared = (proto: object): void => {
    defineMembers(proto, focusMembers());
    defineMembers(proto, styleMembers());
    defineMembers(proto, datasetMembers());
    defineMembers(
      proto,
      handlerMembers((self) => receiverOf(self, HTML_OR_SVG_RULE)),
    );
  };
  const parentNode = (proto: object): void => {
    defineMembers(proto, parentNodeMembers(builders));
    defineMembers(proto, queryMembers(builders));
  };

  defineMembers(all.EventTarget, eventTargetMembers());

  defineMembers(all.Node, nodeTreeMembers(builders));
  defineMembers(all.Node, nodeMutationMembers(builders));
  defineMembers(all.Node, nodeRelationMembers(builders));

  defineMembers(all.CharacterData, characterDataMembers());
  defineMembers(all.CharacterData, nonDocumentTypeChildNodeMembers(builders));
  defineInsertionMembers(all.CharacterData, childNodeMutationMembers(builders));

  defineMembers(all.Text, textMembers());
  defineKind(all.Text, "text");

  defineKind(all.Comment, "comment");

  defineMembers(all.Element, attributeMembers());
  defineMembers(all.Element, nameMembers());
  defineMembers(all.Element, classMembers());
  defineMembers(all.Element, htmlMembers(builders));
  defineMembers(all.Element, layoutMembers());
  defineMembers(all.Element, scrollMembers());
  defineMembers(all.Element, selectorMembers(builders));
  defineMembers(all.Element, pointerCaptureMembers());
  defineMembers(all.Element, nonDocumentTypeChildNodeMembers(builders));
  parentNode(all.Element);
  defineInsertionMembers(
    all.Element,
    parentNodeMutationMembers(builders),
    childNodeMutationMembers(builders),
  );
  defineKind(all.Element, "element");

  defineMembers(all.HTMLElement, clickMembers());
  defineMembers(all.HTMLElement, offsetMembers(builders));
  shared(all.HTMLElement);
  shared(all.ForeignElement);

  parentNode(all.DocumentFragment);
  defineInsertionMembers(all.DocumentFragment, parentNodeMutationMembers(builders));
  defineKind(all.DocumentFragment, "fragment");

  defineMembers(all.Document, documentMembers(builders));
  parentNode(all.Document);
  defineInsertionMembers(all.Document, parentNodeMutationMembers(builders));
  defineMembers(
    all.Document,
    handlerMembers((self) => receiverOf(self, DOCUMENT_RULE)),
  );
  defineKind(all.Document, "document");
}
