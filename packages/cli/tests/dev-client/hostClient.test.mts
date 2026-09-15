// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import path from "node:path";

import { describe, expect, it } from "vitest";

import { HostClient, HostError } from "../../src/dev-client/index.mts";

const mockHost = path.join(import.meta.dirname, "fixtures/mock-host.mts");
const wedgedMockHost = path.join(import.meta.dirname, "fixtures/mock-host-wedged.mts");
const protocolMismatchMockHost = path.join(
  import.meta.dirname,
  "fixtures/mock-host-protocol-mismatch.mts",
);
const appErrorMockHost = path.join(import.meta.dirname, "fixtures/mock-host-app-error.mts");
const unknownMethodMockHost = path.join(
  import.meta.dirname,
  "fixtures/mock-host-unknown-method.mts",
);

describe("HostClient", () => {
  it("spawns the host, calls it, and correlates the response by id", async () => {
    let ready = false;
    const client = new HostClient({
      hostBin: mockHost,
      bundlePath: "bundle.js",
      onReady: () => {
        ready = true;
      },
      onStderr: () => {},
    });

    await client.start();
    await expect(client.call("reload")).resolves.toBeNull();
    await client.stop();

    expect(ready).toBe(true);
  });

  it("relays the host's real stderr and a stray stdout line, distinguishably", async () => {
    const lines: string[] = [];
    const client = new HostClient({
      hostBin: mockHost,
      bundlePath: "bundle.js",
      onStderr: (line) => lines.push(line),
    });

    await client.start();
    await client.call("reload");
    await client.stop();

    expect(lines.some((line) => line.includes("mock host: booted"))).toBe(true);
    expect(lines.some((line) => line.startsWith("[stray stdout] "))).toBe(true);
  });

  it("rejects a call the host answers with a JSON-RPC error", async () => {
    const client = new HostClient({
      hostBin: mockHost,
      bundlePath: "bundle.js",
      onStderr: () => {},
    });
    await client.start();

    await expect(client.call("bogus")).rejects.toThrow(HostError);

    await client.stop();
  });

  it("kills the child once the shutdown deadline passes without it exiting", async () => {
    const client = new HostClient({ hostBin: wedgedMockHost, bundlePath: "bundle.js" });
    await client.start();

    // A wedged app ignores `shutdown` and never exits on its own —
    // resolving here is the kill fallback firing.
    await expect(client.stop(50)).resolves.toBeUndefined();
  }, 5000);

  it("forwards appError to its own callback rather than a generic notification handler", async () => {
    const errors: { message: string; stack: string | null }[] = [];
    const client = new HostClient({
      hostBin: appErrorMockHost,
      bundlePath: "bundle.js",
      onAppError: (error) => errors.push(error),
      onStderr: () => {},
    });

    await client.start();
    await client.stop();

    expect(errors).toEqual([{ message: "boom", stack: "Error: boom\n    at somewhere" }]);
  });

  it("routes a method it doesn't handle to the integration registered for it", async () => {
    const calls: { method: string; params: unknown }[] = [];
    const client = new HostClient({
      hostBin: unknownMethodMockHost,
      bundlePath: "bundle.js",
      integrations: {
        someIntegration: (params) => calls.push({ method: "someIntegration", params }),
      },
      onStderr: () => {},
    });

    await client.start();
    await client.stop();

    expect(calls).toEqual([{ method: "someIntegration", params: { hello: "world" } }]);
  });

  it("logs an unrecognized method to stderr when no integration claims it", async () => {
    const lines: string[] = [];
    const client = new HostClient({
      hostBin: unknownMethodMockHost,
      bundlePath: "bundle.js",
      onStderr: (line) => lines.push(line),
    });

    await client.start();
    await client.stop();

    expect(lines.some((line) => line.includes("someIntegration"))).toBe(true);
  });

  it("stops the host when ready reports a protocol this package wasn't built for, leaving no child behind", async () => {
    const client = new HostClient({
      hostBin: protocolMismatchMockHost,
      bundlePath: "bundle.js",
      onStderr: () => {},
    });

    await client.start();

    // The fixture never exits or answers on its own, so a rejected call
    // here can only mean the mismatch handling killed it.
    await expect(client.call("reload")).rejects.toThrow(/exited/);
  });
});
