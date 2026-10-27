// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expectTypeOf, it } from "vitest";

import type { HANDLER_EVENTS } from "./element/eventHandlers.mts";
import type { EventHandler, IncaElement, IncaInputElement } from "./element/types.mts";
import type {
  IncaCompositionEvent,
  IncaEvent,
  IncaEventMap,
  IncaEventTarget,
  IncaFocusEvent,
  IncaInputEvent,
  IncaKeyboardEvent,
  IncaMouseEvent,
  IncaPointerEvent,
  IncaUIEvent,
  IncaWheelEvent,
  Px,
  StyleProps,
} from "./types.mts";

// These tests pin types. A function under test is never called, so the compiler checks it, and
// `pnpm typecheck` fails on a mismatch. At run time each test only passes.
describe("the event types", () => {
  it("infer the event of a listener function from the event name", () => {
    const listen = (target: IncaEventTarget) =>
      target.addEventListener("keydown", (event) =>
        expectTypeOf(event).toEqualTypeOf<IncaKeyboardEvent>(),
      );
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the event of an object listener from the event name", () => {
    const listen = (target: IncaEventTarget) =>
      target.addEventListener("wheel", {
        handleEvent: (event) => expectTypeOf(event).toEqualTypeOf<IncaWheelEvent>(),
      });
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the base event for a name the host does not dispatch", () => {
    const listen = (target: IncaEventTarget) =>
      target.addEventListener("my-event", (event) =>
        expectTypeOf(event).toEqualTypeOf<IncaEvent>(),
      );
    expectTypeOf(listen).returns.toBeVoid();
  });

  it.each(["beforeinput", "input"] as const)("infer the input event of %s", (name) => {
    const listen = (target: IncaEventTarget) =>
      target.addEventListener(name, (event) => expectTypeOf(event).toEqualTypeOf<IncaInputEvent>());
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the composition event of the composition events", () => {
    const listen = (target: IncaEventTarget) => {
      target.addEventListener("compositionstart", (event) =>
        expectTypeOf(event).toEqualTypeOf<IncaCompositionEvent>(),
      );
      target.addEventListener("compositionupdate", (event) =>
        expectTypeOf(event).toEqualTypeOf<IncaCompositionEvent>(),
      );
      target.addEventListener("compositionend", (event) =>
        expectTypeOf(event).toEqualTypeOf<IncaCompositionEvent>(),
      );
    };
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the base event of change", () => {
    const listen = (target: IncaEventTarget) =>
      target.addEventListener("change", (event) => expectTypeOf(event).toEqualTypeOf<IncaEvent>());
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("type the value of an input element as a string", () => {
    expectTypeOf<IncaInputElement["value"]>().toEqualTypeOf<string>();
  });

  it("infer the event of a removed listener from the event name", () => {
    const listen = (target: IncaEventTarget) =>
      // The listener stays inline so the compiler infers its event type.
      // oxlint-disable-next-line unicorn/no-invalid-remove-event-listener
      target.removeEventListener("focus", (event) =>
        expectTypeOf(event).toEqualTypeOf<IncaFocusEvent>(),
      );
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the event of a handler property from its name", () => {
    const assign = (el: IncaElement) => {
      el.onclick = (event) => expectTypeOf(event).toEqualTypeOf<IncaPointerEvent>();
    };
    expectTypeOf(assign).returns.toBeVoid();
  });

  it("types pointer handler properties as IncaPointerEvent", () => {
    const assign = (el: IncaElement) => {
      el.onpointerdown = (event) => expectTypeOf(event).toEqualTypeOf<IncaPointerEvent>();
      el.ongotpointercapture = (event) => expectTypeOf(event).toEqualTypeOf<IncaPointerEvent>();
    };
    expectTypeOf(assign).returns.toBeVoid();
  });

  it("type this as the target a listener is registered on", () => {
    const listen = (target: IncaEventTarget, el: IncaElement) => {
      target.addEventListener("click", function () {
        expectTypeOf(this).toEqualTypeOf<IncaEventTarget>();
      });
      el.addEventListener("click", function () {
        expectTypeOf(this).toEqualTypeOf<IncaElement>();
      });
      el.addEventListener("my-event", function () {
        expectTypeOf(this).toEqualTypeOf<IncaElement>();
      });
    };
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("type this of a handler property as the element", () => {
    const assign = (el: IncaElement) => {
      el.onclick = function () {
        expectTypeOf(this).toEqualTypeOf<IncaElement>();
      };
    };
    expectTypeOf(assign).returns.toBeVoid();
  });

  it("keep an object in a handler property", () => {
    expectTypeOf<IncaElement["onclick"]>().toEqualTypeOf<
      EventHandler<IncaPointerEvent> | object | null
    >();
  });

  it("accept a listener that declares the fields it reads", () => {
    const listen = (target: IncaEventTarget) => {
      const read = (event: { clientX: number }) => event.clientX;
      target.addEventListener("click", read);
      target.removeEventListener("click", read);
      target.addEventListener("my-event", read);
    };
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("give an event a non-null target and a nullable current target", () => {
    expectTypeOf<IncaEvent["target"]>().toEqualTypeOf<IncaEventTarget>();
    expectTypeOf<IncaEvent["currentTarget"]>().toEqualTypeOf<IncaEventTarget | null>();
    expectTypeOf<ReturnType<IncaEvent["composedPath"]>>().toEqualTypeOf<IncaEventTarget[]>();
  });

  it("leave out the fields of one event kind from the base event", () => {
    expectTypeOf<IncaEvent>().not.toHaveProperty("clientX");
  });

  it("give the base event initEvent", () => {
    expectTypeOf<IncaEvent["initEvent"]>().parameters.toEqualTypeOf<[string, boolean?, boolean?]>();
  });

  it("give the base event the phase constants", () => {
    expectTypeOf<
      [
        IncaEvent["NONE"],
        IncaEvent["CAPTURING_PHASE"],
        IncaEvent["AT_TARGET"],
        IncaEvent["BUBBLING_PHASE"],
      ]
    >().toEqualTypeOf<[0, 1, 2, 3]>();
  });

  it("derive each kind from the kind it extends", () => {
    expectTypeOf<IncaUIEvent>().toExtend<IncaEvent>();
    expectTypeOf<IncaFocusEvent>().toExtend<IncaUIEvent>();
    expectTypeOf<IncaKeyboardEvent>().toExtend<IncaUIEvent>();
    expectTypeOf<IncaInputEvent>().toExtend<IncaUIEvent>();
    expectTypeOf<IncaCompositionEvent>().toExtend<IncaUIEvent>();
    expectTypeOf<IncaMouseEvent>().toExtend<IncaUIEvent>();
    expectTypeOf<IncaPointerEvent>().toExtend<IncaMouseEvent>();
    expectTypeOf<IncaWheelEvent>().toExtend<IncaMouseEvent>();
  });

  it("list a handler for every event but focusin, focusout and the composition events", () => {
    expectTypeOf<(typeof HANDLER_EVENTS)[number]>().toEqualTypeOf<
      Exclude<
        keyof IncaEventMap,
        "focusin" | "focusout" | "compositionstart" | "compositionupdate" | "compositionend"
      >
    >();
  });
});

describe("the style types", () => {
  it("accept the CSS names of the colour and text keys", () => {
    expectTypeOf<StyleProps["color"]>().toEqualTypeOf<number | string | undefined>();
    expectTypeOf<StyleProps["backgroundColor"]>().toEqualTypeOf<number | string | undefined>();
    expectTypeOf<StyleProps["borderRadius"]>().toEqualTypeOf<Px | undefined>();
    expectTypeOf<StyleProps["fontSize"]>().toEqualTypeOf<Px | undefined>();
  });

  it("list CSS names only", () => {
    expectTypeOf<
      Extract<
        keyof StyleProps,
        "flex_direction" | "padding_x" | "margin_y" | "background" | "text_color"
      >
    >().toBeNever();
  });

  it("accept a px string on a px key", () => {
    expectTypeOf<"10px">().toExtend<Px>();
    expectTypeOf<"0">().toExtend<Px>();
    expectTypeOf<StyleProps["width"]>().toEqualTypeOf<Px | "auto" | undefined>();
    expectTypeOf<StyleProps["gap"]>().toEqualTypeOf<Px | undefined>();
    expectTypeOf<StyleProps["paddingTop"]>().toEqualTypeOf<Px | undefined>();
    expectTypeOf<StyleProps["margin"]>().toEqualTypeOf<Px | "auto" | undefined>();
  });

  it("reject a px string with another unit", () => {
    expectTypeOf<{ width: "10em" }>().not.toExtend<StyleProps>();
  });

  it("reject a px string without a unit", () => {
    expectTypeOf<{ gap: "10" }>().not.toExtend<StyleProps>();
  });

  it("accept the flex keyword forms", () => {
    expectTypeOf<"flex-start" | "flex-end">().toExtend<NonNullable<StyleProps["justifyContent"]>>();
    expectTypeOf<"flex-start" | "flex-end">().toExtend<NonNullable<StyleProps["alignItems"]>>();
    expectTypeOf<"row-reverse" | "column-reverse">().toExtend<
      NonNullable<StyleProps["flexDirection"]>
    >();
  });

  it("reject an unknown flex keyword", () => {
    expectTypeOf<{ justifyContent: "flex-middle" }>().not.toExtend<StyleProps>();
  });

  it("accept a decimal string on opacity, flexGrow and flexShrink", () => {
    expectTypeOf<StyleProps["opacity"]>().toEqualTypeOf<number | `${number}` | undefined>();
    expectTypeOf<StyleProps["flexGrow"]>().toEqualTypeOf<number | `${number}` | undefined>();
    expectTypeOf<StyleProps["flexShrink"]>().toEqualTypeOf<number | `${number}` | undefined>();
  });

  it("reject a percentage on opacity", () => {
    expectTypeOf<{ opacity: "50%" }>().not.toExtend<StyleProps>();
  });
});
