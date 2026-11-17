// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createRenderer } from "@vue/runtime-core";
import type { App, Component, ComponentPublicInstance } from "@vue/runtime-core";

import * as core from "../rendererCore.mts";
import { createHeldNodeOps, createNodeOps } from "./nodeOps.mts";
import { createPatchProp } from "./patchProp.mts";
import type { Element, Node } from "../rendererCore.mts";

/**
 * The mounted apps. The host keeps their nodes and this set keeps their
 * containers and handles, so events and `document.activeElement` find the elements
 * of an app that nothing else references.
 */
const mounted = new Set<App<Element>>();

/**
 * `@vue/runtime-core`'s `App`, with `mount` also callable with no argument
 * to target the host's own root container. `use`, `mixin`, `component` and
 * `directive` return an `IncaApp`.
 */
export type IncaApp = App<Element> & {
  mount: (rootContainer?: Element) => ComponentPublicInstance;
};

/**
 * Creates a Vue app whose root mounts on an {@link Element} host
 * handle. `app.mount()` with no argument targets the host's root
 * container and returns the root component instance. `app.unmount`,
 * `app.use` and the rest behave as `@vue/runtime-core` documents them.
 * @param rootComponent - the component to mount as the app's root
 * @param rootProps - props to pass to that root component
 * @returns the created app
 */
export function createIncaApp(
  rootComponent: Component,
  rootProps?: Record<string, unknown> | null,
): IncaApp {
  const options = {
    ...createNodeOps(core),
    patchProp: createPatchProp(core),
  };
  const renderer = createRenderer<Node, Element>(options);
  // KeepAlive, Suspense and Teleport create nodes they insert later; holding them keeps the host
  // from freeing them. They read `options` after `createRenderer`, so this assignment reaches them.
  Object.assign(options, createHeldNodeOps(core, options));
  const app = renderer.createApp(rootComponent, rootProps);
  // Captured before the override, or the replacement would call itself.
  const mountAt = app.mount.bind(app);
  const unmountAt = app.unmount.bind(app);

  return Object.assign(app, {
    mount: (rootContainer?: Element) => {
      const instance = mountAt(rootContainer ?? core.rootElement());
      mounted.add(app);
      return instance;
    },
    unmount: () => {
      unmountAt();
      mounted.delete(app);
    },
  });
}
