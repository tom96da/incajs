// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { datasetDelete, datasetGet, datasetKeys, datasetSet, datasetTarget } from "../tree.mts";
import { attributeText } from "./attributes.mts";
import type { NodeId } from "../types.mts";
import type { DOMStringMap, IncaElement } from "./types.mts";

/** The members {@link datasetMembers} builds. */
export type DatasetMembers = Pick<IncaElement, "dataset">;

/**
 * Builds the `dataset` member of the element that the host knows as `id`. The object is a proxy
 * whose traps forward to the dataset natives. A key that no attribute answers goes to the target
 * and its prototype.
 * @param id - the native node's id
 * @returns the members, with `dataset` as a getter that creates the object once
 */
export function datasetMembers(id: NodeId): DatasetMembers {
  let map: DOMStringMap | undefined;
  return {
    get dataset() {
      map ??= new Proxy(datasetTarget(), {
        get(target, key, receiver) {
          const value = typeof key === "string" ? datasetGet(id, key) : undefined;
          return typeof value === "string" ? value : Reflect.get(target, key, receiver);
        },
        set(target, key, value, receiver) {
          if (typeof key !== "string") return Reflect.set(target, key, value, receiver);
          datasetSet(id, key, attributeText(value));
          return true;
        },
        deleteProperty(target, key) {
          if (typeof key !== "string") return Reflect.deleteProperty(target, key);
          return datasetDelete(id, key);
        },
        defineProperty(target, key, descriptor) {
          if (typeof key !== "string") return Reflect.defineProperty(target, key, descriptor);
          if (!("value" in descriptor)) return false;
          datasetSet(id, key, attributeText(descriptor.value));
          return true;
        },
        preventExtensions: () => false,
        has: (target, key) =>
          (typeof key === "string" && datasetKeys(id).includes(key)) || Reflect.has(target, key),
        ownKeys: (target) => [...datasetKeys(id), ...Reflect.ownKeys(target)],
        getOwnPropertyDescriptor(target, key) {
          const value = typeof key === "string" ? datasetGet(id, key) : undefined;
          if (typeof value !== "string") return Reflect.getOwnPropertyDescriptor(target, key);
          return { value, writable: true, enumerable: true, configurable: true };
        },
      }) as DOMStringMap;
      return map;
    },
  };
}
