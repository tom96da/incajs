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
});
