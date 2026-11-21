// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, describe, expect, it } from "vitest";

import { createIncaDevTransport } from "./createIncaDevTransport.mts";

afterEach(() => {
  globalThis.__inca_dev__ = undefined;
});

describe("createIncaDevTransport", () => {
  it("names the development protocol when no host serves it", () => {
    const transport = createIncaDevTransport();
    globalThis.__inca_dev__ = undefined;

    expect(() => transport.connect({ onMessage() {}, onDisconnection() {} })).toThrow(
      "a host that serves the development protocol",
    );
    expect(() => transport.send({ type: "ping" })).toThrow(
      "a host that serves the development protocol",
    );
  });

  describe("while held", () => {
    const invoke = { type: "custom", event: "vite:invoke", data: { id: "response:1" } };
    const changed = { type: "custom", event: "file-changed", data: { file: "/a.css" } };
    const update = { type: "update", updates: [] };

    /** Connects a transport whose runner records what it receives, and holds it. */
    function held() {
      const received: unknown[] = [];
      let fileChanges = 0;
      globalThis.__inca_dev__ = { send() {} };
      const transport = createIncaDevTransport(() => fileChanges++);
      void transport.connect({
        onMessage: (payload) => received.push(payload),
        onDisconnection() {},
      });
      transport.hold();
      const receive = (payload: object) =>
        globalThis.__inca_dev__?.receive?.("vite", JSON.stringify(payload));
      return { transport, received, receive, fileChanges: () => fileChanges };
    }

    it("passes the replies to the runner's calls and counts a changed file at once", () => {
      const { received, receive, fileChanges } = held();

      receive(changed);
      receive(invoke);

      expect(received).toEqual([invoke]);
      expect(fileChanges()).toBe(1);
    });

    it("delivers the held payloads in order on a release that delivers", () => {
      const { transport, received, receive } = held();

      receive(update);
      receive(changed);
      transport.release(true);
      receive(invoke);

      expect(received).toEqual([update, changed, invoke]);
    });

    it("drops the held payloads on a release that does not deliver", () => {
      const { transport, received, receive } = held();

      receive(update);
      transport.release(false);
      receive(changed);

      expect(received).toEqual([changed]);
    });
  });
});
