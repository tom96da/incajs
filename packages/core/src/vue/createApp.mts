// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createRenderer } from "@vue/runtime-core";
import type { App, Component, ComponentPublicInstance } from "@vue/runtime-core";

import * as core from "../rendererCore.mts";
import { createNodeOps } from "./nodeOps.mts";
import { createPatchProp } from "./patchProp.mts";
import type { IncaElement, IncaNode } from "./nodeOps.mts";

/**
 * `@vue/runtime-core`'s `App`, with `mount` additionally callable with no
 * argument to target the host's own root container. An intersection rather
 * than an `Omit`: `App`'s `this`-returning methods (`use`, `mixin`,
 * `component`, `directive`) have to keep this wider `mount` when chained.
 */
export type IncaApp = App<IncaElement> & {
  mount: (rootContainer?: IncaElement) => ComponentPublicInstance;
};

/**
 * Creates a Vue app whose root mounts against a {@link IncaElement} host
 * handle instead of a DOM element. `app.mount()` with no argument targets
 * the host's root container; `app.unmount`, `app.use`, etc. all behave
 * exactly as `@vue/runtime-core` itself documents them.
 * @param rootComponent - the component to mount as the app's root
 * @param rootProps - props to pass to that root component
 * @returns the created app
 */
export function createIncaApp(
  rootComponent: Component,
  rootProps?: Record<string, unknown> | null,
): IncaApp {
  const renderer = createRenderer<IncaNode, IncaElement>({
    ...createNodeOps(core),
    patchProp: createPatchProp(core),
  });
  const app = renderer.createApp(rootComponent, rootProps);
  // Captured before the override, or the replacement would call itself.
  const mountAt = app.mount.bind(app);

  // The host's root container, with the same `.focus()`/`.blur()`/`tabIndex`
  // `createNodeOps`'s own elements get.
  function rootElement(): IncaElement {
    const id = core.rootNodeId();
    let tabIndex = -1;
    return {
      id,
      kind: "element",
      parent: null,
      children: [],
      focus: () => core.focus(id),
      blur: () => core.blur(id),
      get tabIndex() {
        return tabIndex;
      },
      set tabIndex(value: number) {
        tabIndex = Math.trunc(value);
        core.setAttribute(id, "tabindex", tabIndex);
      },
    };
  }

  return Object.assign(app, {
    mount: (rootContainer?: IncaElement) => mountAt(rootContainer ?? rootElement()),
  });
}
