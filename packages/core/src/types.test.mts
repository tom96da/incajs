// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expectTypeOf, it } from "vitest";

import type { DOMRect, DOMRectReadOnly } from "./element/geometry.mts";
import type { Node } from "./element/nodeTypes.mts";
import type {
  Comment,
  CSSStyleDeclaration,
  DocumentFragment,
  DOMStringMap,
  DOMTokenList,
  Element,
  EventHandler,
  HTMLElement,
  HTMLInputElement,
  Text,
} from "./element/types.mts";
import type {
  AddEventListenerOptions,
  CompositionEvent,
  Document,
  Event,
  EventListenerOptions,
  EventTarget,
  FocusEvent,
  InputEvent,
  KeyboardEvent,
  MouseEvent,
  PointerEvent,
  Px,
  StyleProps,
  UIEvent,
  WheelEvent,
  Window,
} from "./types.mts";

// These tests pin types. A function under test is never called, so the compiler checks it, and
// `pnpm typecheck` fails on a mismatch. At run time each test only passes.
describe("the event types", () => {
  it("infer the event of a listener function from the event name", () => {
    const listen = (target: EventTarget) =>
      target.addEventListener("keydown", (event) =>
        expectTypeOf(event).toEqualTypeOf<KeyboardEvent>(),
      );
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the keyboard event of a keypress listener", () => {
    const listen = (target: EventTarget) =>
      target.addEventListener("keypress", (event) =>
        expectTypeOf(event).toEqualTypeOf<KeyboardEvent>(),
      );
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the event of an object listener from the event name", () => {
    const listen = (target: EventTarget) =>
      target.addEventListener("wheel", {
        handleEvent: (event) => expectTypeOf(event).toEqualTypeOf<WheelEvent>(),
      });
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the pointer event of click", () => {
    const listen = (target: EventTarget) =>
      target.addEventListener("click", (event) => expectTypeOf(event.pointerId).toBeNumber());
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the base event for a name the host does not dispatch", () => {
    const listen = (target: EventTarget) =>
      target.addEventListener("my-event", (event) => expectTypeOf(event).toEqualTypeOf<Event>());
    expectTypeOf(listen).returns.toBeVoid();
  });

  it.each(["beforeinput", "input"] as const)("infer the input event of %s", (name) => {
    const listen = (target: EventTarget) =>
      target.addEventListener(name, (event) => expectTypeOf(event).toEqualTypeOf<InputEvent>());
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the composition event of the composition events", () => {
    const listen = (target: EventTarget) => {
      target.addEventListener("compositionstart", (event) =>
        expectTypeOf(event).toEqualTypeOf<CompositionEvent>(),
      );
      target.addEventListener("compositionupdate", (event) =>
        expectTypeOf(event).toEqualTypeOf<CompositionEvent>(),
      );
      target.addEventListener("compositionend", (event) =>
        expectTypeOf(event).toEqualTypeOf<CompositionEvent>(),
      );
    };
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the base event of change", () => {
    const listen = (target: EventTarget) =>
      target.addEventListener("change", (event) => expectTypeOf(event).toEqualTypeOf<Event>());
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("type the value of an input element as a string", () => {
    expectTypeOf<HTMLInputElement["value"]>().toEqualTypeOf<string>();
  });

  it("infer the event of a removed listener from the event name", () => {
    const listen = (target: EventTarget) =>
      // The listener stays inline so the compiler infers its event type.
      // oxlint-disable-next-line unicorn/no-invalid-remove-event-listener
      target.removeEventListener("focus", (event) =>
        expectTypeOf(event).toEqualTypeOf<FocusEvent>(),
      );
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("infer the event of a handler property from its name", () => {
    const assign = (el: HTMLElement) => {
      el.onclick = (event) => expectTypeOf(event).toEqualTypeOf<PointerEvent>();
    };
    expectTypeOf(assign).returns.toBeVoid();
  });

  it("types pointer handler properties as PointerEvent", () => {
    const assign = (el: HTMLElement) => {
      el.onpointerdown = (event) => expectTypeOf(event).toEqualTypeOf<PointerEvent>();
      el.ongotpointercapture = (event) => expectTypeOf(event).toEqualTypeOf<PointerEvent>();
    };
    expectTypeOf(assign).returns.toBeVoid();
  });

  it("type this as the target a listener is registered on", () => {
    const listen = (target: EventTarget, el: Element) => {
      target.addEventListener("click", function () {
        expectTypeOf(this).toEqualTypeOf<EventTarget>();
      });
      el.addEventListener("click", function () {
        expectTypeOf(this).toEqualTypeOf<Element>();
      });
      el.addEventListener("my-event", function () {
        expectTypeOf(this).toEqualTypeOf<Element>();
      });
    };
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("type this of a handler property as the element", () => {
    const assign = (el: HTMLElement) => {
      el.onclick = function () {
        expectTypeOf(this).toEqualTypeOf<HTMLElement>();
      };
    };
    expectTypeOf(assign).returns.toBeVoid();
  });

  it("keep an object in a handler property", () => {
    expectTypeOf<HTMLElement["onclick"]>().toEqualTypeOf<
      EventHandler<PointerEvent, HTMLElement> | object | null
    >();
  });

  it("accept a listener that declares the fields it reads", () => {
    const listen = (target: EventTarget) => {
      const read = (event: { clientX: number }) => event.clientX;
      target.addEventListener("click", read);
      target.removeEventListener("click", read);
      target.addEventListener("my-event", read);
    };
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("give an event a nullable target, source element and current target", () => {
    expectTypeOf<Event["target"]>().toEqualTypeOf<EventTarget | null>();
    expectTypeOf<Event["srcElement"]>().toEqualTypeOf<EventTarget | null>();
    expectTypeOf<Event["currentTarget"]>().toEqualTypeOf<EventTarget | null>();
    expectTypeOf<ReturnType<Event["composedPath"]>>().toEqualTypeOf<EventTarget[]>();
  });

  it("make a listener guard the target before it reads it", () => {
    const listen = (el: Element) => {
      el.addEventListener("click", (event) => {
        // @ts-expect-error the target can be null
        event.target.addEventListener("x", () => undefined);
        event.target?.addEventListener("x", () => undefined);
      });
    };
    expectTypeOf(listen).returns.toBeVoid();
  });

  it("type the text content of an element as nullable", () => {
    const read = (el: Element) => {
      const text: string | null = el.textContent;
      // @ts-expect-error textContent can be null
      const length: number = el.textContent.length;
      return [text, length];
    };
    expectTypeOf(read).returns.toBeArray();
  });

  it("take an event object in dispatchEvent and no plain object", () => {
    const dispatch = (target: EventTarget, event: Event) => {
      target.dispatchEvent(event);
      // @ts-expect-error a plain object is not an event
      target.dispatchEvent({ type: "x" });
    };
    expectTypeOf(dispatch).returns.toBeVoid();
  });

  it("leave out the fields of one event kind from the base event", () => {
    expectTypeOf<Event>().not.toHaveProperty("clientX");
  });

  it("give the base event initEvent", () => {
    expectTypeOf<Event["initEvent"]>().parameters.toEqualTypeOf<[string, boolean?, boolean?]>();
  });

  it("give the base event the phase constants", () => {
    expectTypeOf<
      [Event["NONE"], Event["CAPTURING_PHASE"], Event["AT_TARGET"], Event["BUBBLING_PHASE"]]
    >().toEqualTypeOf<[0, 1, 2, 3]>();
  });

  it("derive each kind from the kind it extends", () => {
    expectTypeOf<UIEvent>().toExtend<Event>();
    expectTypeOf<FocusEvent>().toExtend<UIEvent>();
    expectTypeOf<KeyboardEvent>().toExtend<UIEvent>();
    expectTypeOf<InputEvent>().toExtend<UIEvent>();
    expectTypeOf<CompositionEvent>().toExtend<UIEvent>();
    expectTypeOf<MouseEvent>().toExtend<UIEvent>();
    expectTypeOf<PointerEvent>().toExtend<MouseEvent>();
    expectTypeOf<WheelEvent>().toExtend<MouseEvent>();
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
    expectTypeOf<StyleProps["opacity"]>().toEqualTypeOf<
      number | `${number}` | `${number}%` | undefined
    >();
    expectTypeOf<StyleProps["flexGrow"]>().toEqualTypeOf<number | `${number}` | undefined>();
    expectTypeOf<StyleProps["flexShrink"]>().toEqualTypeOf<number | `${number}` | undefined>();
  });

  it("accept a percentage on opacity", () => {
    expectTypeOf<{ opacity: "50%" }>().toExtend<StyleProps>();
  });

  it("reject a percentage on flexGrow", () => {
    expectTypeOf<{ flexGrow: "50%" }>().not.toExtend<StyleProps>();
  });
});

describe("the style member", () => {
  it("reads a CSSStyleDeclaration on an element", () => {
    expectTypeOf<HTMLElement["style"]>().toEqualTypeOf<CSSStyleDeclaration>();
  });

  it("types a host-only style property as a string", () => {
    expectTypeOf<CSSStyleDeclaration["textColor"]>().toEqualTypeOf<string>();
  });

  it("types a style property as a string in both spellings", () => {
    expectTypeOf<CSSStyleDeclaration["backgroundColor"]>().toEqualTypeOf<string>();
    expectTypeOf<CSSStyleDeclaration["background-color"]>().toEqualTypeOf<string>();
  });
});

describe("the dataset type", () => {
  it("reads a key as a string or undefined", () => {
    expectTypeOf<HTMLElement["dataset"]["x"]>().toEqualTypeOf<string | undefined>();
  });
});

describe("the host interface globals", () => {
  it("type the prototype of CSSStyleDeclaration as the exported interface", () => {
    expectTypeOf<
      typeof globalThis.CSSStyleDeclaration.prototype
    >().toEqualTypeOf<CSSStyleDeclaration>();
  });

  it("type the prototype of DOMStringMap as the exported interface", () => {
    expectTypeOf<typeof globalThis.DOMStringMap.prototype>().toEqualTypeOf<DOMStringMap>();
  });

  it("type the prototype of DOMTokenList as the exported interface", () => {
    expectTypeOf<typeof globalThis.DOMTokenList.prototype>().toEqualTypeOf<DOMTokenList>();
  });
});

describe("the modern mutation members", () => {
  it("take nodes and strings and return void", () => {
    expectTypeOf<Element["append"]>().toBeCallableWith("s", {} as Text, {} as Element);
    expectTypeOf<Element["append"]>().returns.toBeVoid();
  });

  it("give a text node no append", () => {
    expectTypeOf<Text>().not.toHaveProperty("append");
  });

  it("give a comment remove and no append", () => {
    expectTypeOf<Comment>().toHaveProperty("remove");
    expectTypeOf<Comment>().not.toHaveProperty("append");
  });
});

describe("the abort globals", () => {
  it("type a controller and its signal", () => {
    expectTypeOf<AbortController["signal"]>().toEqualTypeOf<AbortSignal>();
    expectTypeOf<AbortController["abort"]>().parameters.toEqualTypeOf<[reason?: any]>();
  });

  it("type the members of a signal and its static methods", () => {
    expectTypeOf<AbortSignal["aborted"]>().toEqualTypeOf<boolean>();
    expectTypeOf<AbortSignal["reason"]>().toBeAny();
    expectTypeOf<AbortSignal["throwIfAborted"]>().returns.toBeVoid();
    expectTypeOf<typeof AbortSignal.timeout>().parameters.toEqualTypeOf<[milliseconds: number]>();
    expectTypeOf<typeof AbortSignal.any>().parameters.toEqualTypeOf<[signals: AbortSignal[]]>();
    expectTypeOf<typeof AbortSignal.abort>().returns.toEqualTypeOf<AbortSignal>();
  });

  it("type onabort with the signal as this and the base event", () => {
    expectTypeOf<AbortSignal["onabort"]>().toEqualTypeOf<
      ((this: AbortSignal, ev: Event) => any) | null
    >();
  });

  it("infer the base event of an abort listener", () => {
    const listen = (signal: AbortSignal) =>
      signal.addEventListener("abort", (event) => expectTypeOf(event).toEqualTypeOf<Event>());
    expectTypeOf(listen).returns.toBeVoid();
  });
});

describe("the signal option", () => {
  it("is the global AbortSignal", () => {
    expectTypeOf<AddEventListenerOptions["signal"]>().toEqualTypeOf<AbortSignal | undefined>();
  });

  it("takes a signal of a controller and of any() in a listener call", () => {
    const listen = (target: EventTarget, controller: AbortController) => {
      target.addEventListener("click", () => undefined, { signal: controller.signal });
      target.addEventListener("click", () => undefined, {
        signal: AbortSignal.any([controller.signal]),
      });
    };
    expectTypeOf(listen).returns.toBeVoid();
  });
});

describe("the node types", () => {
  it("give each interface the node type of its kind", () => {
    expectTypeOf<Element["nodeType"]>().toEqualTypeOf<1>();
    expectTypeOf<Text["nodeType"]>().toEqualTypeOf<3>();
    expectTypeOf<Comment["nodeType"]>().toEqualTypeOf<8>();
    expectTypeOf<Document["nodeType"]>().toEqualTypeOf<9>();
    expectTypeOf<DocumentFragment["nodeType"]>().toEqualTypeOf<11>();
    expectTypeOf<Node["nodeType"]>().toEqualTypeOf<1 | 3 | 8 | 9 | 11>();
  });

  it("type the parent node as any node", () => {
    expectTypeOf<Node["parentNode"]>().toEqualTypeOf<Node | null>();
  });

  it("type the view of a UI event as a nullable window", () => {
    expectTypeOf<UIEvent["view"]>().toEqualTypeOf<Window | null>();
  });

  it("export the option of removeEventListener", () => {
    expectTypeOf<EventListenerOptions>().toEqualTypeOf<{ capture?: boolean }>();
    expectTypeOf<AddEventListenerOptions>().toExtend<EventListenerOptions>();
  });
});

describe("the rect type", () => {
  it("lets a script assign the position and size of a bounding rect", () => {
    const move = (el: Element) => {
      const rect = el.getBoundingClientRect();
      rect.x = 1;
      rect.width = 1;
      // @ts-expect-error the edges are read-only
      rect.top = 1;
    };
    expectTypeOf(move).returns.toBeVoid();
  });

  it("gives the read-only rect no writable position", () => {
    const move = (rect: DOMRectReadOnly) => {
      // @ts-expect-error the read-only rect cannot be assigned
      rect.x = 1;
    };
    expectTypeOf(move).returns.toBeVoid();
    expectTypeOf<DOMRect>().toExtend<DOMRectReadOnly>();
  });
});
