// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createRenderer, h, nextTick, ref } from "@vue/runtime-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { nativeStub } from "../../tests/setup.mts";
import * as rendererCore from "../rendererCore.mts";
import { createNodeOps } from "./nodeOps.mts";
import { createPatchProp } from "./patchProp.mts";
import type { RendererCore, Element } from "../rendererCore.mts";

/** What the host calls: it passes the event object first. */
type Callback = (...args: unknown[]) => void;

/** The function a registration call received, callable with any event. */
const listenerOf = (call: readonly unknown[]): ((...args: unknown[]) => void) =>
  call[2] as (...args: unknown[]) => void;

const core: RendererCore = {
  ...rendererCore,
  addEventListener: vi.fn<RendererCore["addEventListener"]>(),
  removeEventListener: vi.fn<RendererCore["removeEventListener"]>(),
  latestEventId: vi.fn<() => number>(() => 0),
  isRemoved: () => false,
};

const patchProp = createPatchProp(core);

const BUBBLE = { capture: false, passive: false };

/** An element holds the id of its node, which is all that patchProp reads. */
const makeEl = (): Element => ({ __node_id: 1 }) as unknown as Element;

let el = makeEl();

beforeEach(() => {
  vi.clearAllMocks();
  el = makeEl();
});

describe("a listener attached while an event is handled", () => {
  let latest = 0;
  const set = (key: string, value: unknown, target = el): void =>
    patchProp(target, key, null, value, undefined, null);
  // Delivers event `id` to the newest registration for (nodeId, "click").
  const send = (id: number | undefined, nodeId = 1): void => {
    if (id !== undefined) latest = Math.max(latest, id);
    const calls = vi.mocked(core.addEventListener).mock.calls;
    listenerOf(calls.findLast(([target, name]) => target === nodeId && name === "click")!)(
      id === undefined ? {} : { eventId: id },
    );
  };
  const spy = () => vi.fn<() => void>();

  beforeEach(() => {
    latest = 0;
    vi.mocked(core.latestEventId).mockImplementation(() => latest);
  });
  afterEach(() => {
    vi.mocked(core.latestEventId).mockImplementation(() => 0);
  });

  it("is skipped for that event and runs for the next", () => {
    const late = spy();
    latest = 5;
    set("onClick", late);
    send(5);
    expect(late).not.toHaveBeenCalled();
    send(6);
    expect(late).toHaveBeenCalledTimes(1);
  });

  it("runs for the first event when it was attached before any event", () => {
    const early = spy();
    set("onClick", early);
    send(1);
    expect(early).toHaveBeenCalledTimes(1);
  });

  it("runs for an event with no eventId", () => {
    const late = spy();
    latest = 5;
    set("onClick", late);
    send(undefined);
    expect(late).toHaveBeenCalledTimes(1);
  });

  it("is skipped while an older listener on the same event still runs", () => {
    const old = spy();
    const late = spy();
    latest = 4;
    set("onClick", old);
    latest = 5;
    set("onClickOnce", late);
    send(5);
    expect(old).toHaveBeenCalledTimes(1);
    expect(late).not.toHaveBeenCalled();
  });

  it("keeps its attach point when a re-render replaces its function", () => {
    const first = spy();
    const next = spy();
    latest = 4;
    set("onClick", first);
    latest = 5;
    set("onClick", next);
    send(5);
    expect(first).not.toHaveBeenCalled();
    expect(next).toHaveBeenCalledTimes(1);

    const late = spy();
    const replaced = spy();
    set("onClickOnce", late);
    set("onClickOnce", replaced);
    send(5);
    expect(late).not.toHaveBeenCalled();
    expect(replaced).not.toHaveBeenCalled();
    send(6);
    expect(replaced).toHaveBeenCalledTimes(1);
  });

  it("stays unspent when it is a .once listener that skipped an event", () => {
    const once = spy();
    latest = 5;
    set("onClickOnce", once);
    send(5);
    expect(once).not.toHaveBeenCalled();
    expect(core.removeEventListener).not.toHaveBeenCalled();
    send(6);
    send(7);
    expect(once).toHaveBeenCalledTimes(1);
    expect(core.removeEventListener).toHaveBeenCalledExactlyOnceWith(
      1,
      "click",
      expect.any(Function),
      BUBBLE,
    );
  });

  it("is judged against the event being dispatched when dispatches nest", () => {
    const [attachedOuter, attachedInner] = [spy(), spy()];
    let depth = 0;
    latest = 4;
    set("onClick", () => {
      if (depth++ > 0) return set("onClickOnceOnce", attachedInner);
      set("onClickOnce", attachedOuter);
      send(6);
    });
    send(5);
    expect(attachedOuter).toHaveBeenCalledTimes(1);
    expect(attachedInner).not.toHaveBeenCalled();
    send(6);
    expect(attachedInner).not.toHaveBeenCalled();
    send(7);
    expect(attachedInner).toHaveBeenCalledTimes(1);
  });

  it("is skipped by the ancestor dispatch of the event that caused the re-render", async () => {
    let id = 1;
    vi.stubGlobal(
      "__inca_native__",
      nativeStub({
        createNode: () => ++id,
        insertBefore: () => {},
        linkedNode: () => null,
      } as never),
    );
    const renderer = createRenderer({ ...createNodeOps(core), patchProp });
    const armed = ref(false);
    const onAncestor = spy();
    const App = {
      render: () =>
        h("view", { onClick: armed.value ? onAncestor : undefined }, [
          h("view", { onClick: () => (armed.value = true) }),
        ]),
    };
    renderer.createApp(App).mount(el);
    const ancestor = 2;
    const child = 3;

    // The host runs the child, drains jobs, then runs the ancestor.
    send(1, child);
    await nextTick();
    send(1, ancestor);
    expect(onAncestor).not.toHaveBeenCalled();

    send(2, child);
    await nextTick();
    send(2, ancestor);
    expect(onAncestor).toHaveBeenCalledTimes(1);
  });
});

describe("with the event tracker of the events layer", () => {
  let nextCallbackId = 0;
  const addEventListener = vi.fn<(nodeId: number, event: string, callback: Callback) => number>(
    () => nextCallbackId++,
  );
  const real = createPatchProp({
    ...core,
    addEventListener: rendererCore.addEventListener,
    removeEventListener: rendererCore.removeEventListener,
    latestEventId: rendererCore.latestEventId,
  });
  const other = { ...makeEl(), __node_id: 2 };

  beforeEach(() => {
    addEventListener.mockClear();
    vi.stubGlobal(
      "__inca_native__",
      nativeStub({
        addEventListener,
        removeEventListener: vi.fn<() => boolean>(() => true),
      } as never),
    );
  });

  // Delivers event `eventId` to the newest registration for `nodeId`.
  const send = (nodeId: number, eventId: number): void => {
    const index = addEventListener.mock.calls.findLastIndex(([id]) => id === nodeId);
    addEventListener.mock.calls[index]![2]({ eventId });
  };

  it("skips a listener attached by an earlier node's handler until the next event", () => {
    const late = vi.fn<() => void>();
    const event = rendererCore.latestEventId() + 1;
    real(
      el,
      "onClick",
      null,
      () => real(other, "onClick", null, late, undefined, null),
      undefined,
      null,
    );

    send(1, event);
    send(2, event);
    expect(late).not.toHaveBeenCalled();
    send(2, event + 1);
    expect(late).toHaveBeenCalledTimes(1);
  });
});
