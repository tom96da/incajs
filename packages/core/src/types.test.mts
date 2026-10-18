// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expectTypeOf, it } from "vitest";

import type { EventHandler, IncaElement } from "./element/types.mts";
import type { IncaEvent, IncaEventTarget } from "./types.mts";

// These tests pin types. A function under test is never called, so the compiler checks it, and
// `pnpm typecheck` fails on a mismatch. At run time each test only passes.
describe("the event types", () => {
  it("infer the event of a listener function", () => {
    const listen = (target: IncaEventTarget) =>
      target.addEventListener("click", (event) => expectTypeOf(event).toEqualTypeOf<IncaEvent>());
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the event of an object listener", () => {
    const listen = (target: IncaEventTarget) =>
      target.addEventListener("click", {
        handleEvent: (event) => expectTypeOf(event).toEqualTypeOf<IncaEvent>(),
      });
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the event of a handler property", () => {
    const assign = (el: IncaElement) => {
      el.onclick = (event) => expectTypeOf(event).toEqualTypeOf<IncaEvent>();
    };
    expectTypeOf(assign).returns.toBeVoid();
  });

  it("type this as the current target", () => {
    const listen = (target: IncaEventTarget, el: IncaElement) => {
      target.addEventListener("click", function () {
        expectTypeOf(this).toEqualTypeOf<IncaEventTarget>();
      });
      el.onclick = function () {
        expectTypeOf(this).toEqualTypeOf<IncaElement>();
      };
    };
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("keep an object in a handler property", () => {
    expectTypeOf<IncaElement["onclick"]>().toEqualTypeOf<EventHandler | object | null>();
  });

  it("accept a listener that declares the fields it reads", () => {
    const listen = (target: IncaEventTarget) => {
      const read = (event: { clientX: number }) => event.clientX;
      target.addEventListener("click", read);
      target.removeEventListener("click", read);
    };
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("give an event a non-null target and a nullable current target", () => {
    expectTypeOf<IncaEvent["target"]>().toEqualTypeOf<IncaEventTarget>();
    expectTypeOf<IncaEvent["currentTarget"]>().toEqualTypeOf<IncaEventTarget | null>();
    expectTypeOf<ReturnType<IncaEvent["composedPath"]>>().toEqualTypeOf<IncaEventTarget[]>();
  });

  it("leave out the fields of one event kind", () => {
    expectTypeOf<IncaEvent>().not.toHaveProperty("clientX");
  });
});
