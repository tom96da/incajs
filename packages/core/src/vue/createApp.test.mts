// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { setFlagsFromString } from "node:v8";
import { runInNewContext } from "node:vm";

import { h, reactive } from "@vue/runtime-core";
import { describe, expect, it } from "vitest";

import { window } from "../rendererCore.mts";
import { createIncaApp } from "./createApp.mts";
import type { IncaElement } from "../rendererCore.mts";

let nextId = 2;
globalThis.__inca_native__ = {
  rootNodeId: () => 1,
  createNode: () => nextId++,
  insertBefore: () => {},
  destroyNode: () => [],
} as never;

describe("createIncaApp", () => {
  it("keeps the same root handle inside reactive()", () => {
    const app = createIncaApp({ render: () => h("div") });
    app.mount();
    const root = (app as unknown as { _container: IncaElement })._container;

    expect(root.__node_id).toBe(1);
    expect(reactive(root)).toBe(root);
    app.unmount();
  });

  it("mounts into the element that events already carry for the host root", () => {
    const [resolved] = globalThis.__inca_targets__([1]);
    const app = createIncaApp({ render: () => h("div") });

    app.mount();

    expect((app as unknown as { _container: IncaElement })._container).toBe(resolved);
    app.unmount();
  });
});

describe("window under reactive()", () => {
  it("stays itself inside reactive()", () => {
    expect(reactive({ w: window }).w).toBe(window);
  });
});

describe("createIncaApp mounted containers", () => {
  setFlagsFromString("--expose-gc");
  const gc = runInNewContext("gc") as () => void;

  /** Each round runs in its own timer tick, since a deref keeps its target alive until the job ends. */
  async function collectGarbage(): Promise<void> {
    for (let i = 0; i < 10; i++) {
      gc();
      await new Promise((resolve) => setTimeout(resolve, 5));
    }
  }

  it("keeps the container of a mounted app until unmount", async () => {
    const [app, container] = (() => {
      const mountedApp = createIncaApp({ render: () => h("div") });
      mountedApp.mount();
      const root = (mountedApp as unknown as { _container: IncaElement })._container;
      return [new WeakRef(mountedApp), new WeakRef(root)];
    })();

    await collectGarbage();
    expect(container.deref()).toBeDefined();

    app.deref()!.unmount();
    await collectGarbage();
    expect(container.deref()).toBeUndefined();
  });
});
