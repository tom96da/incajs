// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { h, reactive } from "@vue/runtime-core";
import { beforeEach, describe, expect, it } from "vitest";

import { installFakeHost } from "../../tests/fakeHost.mts";
import { collectGarbage, collectSentinel } from "../../tests/gc.mts";
import { window } from "../rendererCore.mts";
import { createIncaApp } from "./createApp.mts";
import type { FakeHost } from "../../tests/fakeHost.mts";
import type { Element } from "../rendererCore.mts";

let host: FakeHost;

beforeEach(() => {
  host = installFakeHost();
});

describe("createIncaApp", () => {
  it("keeps the same root handle inside reactive()", () => {
    const app = createIncaApp({ render: () => h("div") });
    app.mount();
    const root = (app as unknown as { _container: Element })._container;

    expect(root.__node_id).toBe(host.ROOT);
    expect(reactive(root)).toBe(root);
    app.unmount();
  });

  it("mounts into the element that events already carry for the host root", () => {
    const [resolved] = globalThis.__inca_targets__([host.ROOT]);
    const app = createIncaApp({ render: () => h("div") });

    app.mount();

    expect((app as unknown as { _container: Element })._container).toBe(resolved);
    app.unmount();
  });
});

describe("window under reactive()", () => {
  it("stays itself inside reactive()", () => {
    expect(reactive({ w: window }).w).toBe(window);
  });
});

describe("createIncaApp mounted containers", () => {
  it("keeps the container of a mounted app until unmount", async () => {
    const [app, container] = (() => {
      const mountedApp = createIncaApp({ render: () => h("div") });
      mountedApp.mount();
      const root = (mountedApp as unknown as { _container: Element })._container;
      return [new WeakRef(mountedApp), new WeakRef(root)];
    })();

    await collectSentinel();
    expect(container.deref()).toBeDefined();

    app.deref()!.unmount();
    await collectGarbage(() => container.deref() === undefined);
    expect(container.deref()).toBeUndefined();
  });
});
