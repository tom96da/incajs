// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { defaultIdentifier, defaultProductName, slugify, unscopedName } from "./defaults.mts";

describe("unscopedName", () => {
  it("strips an npm scope", () => {
    expect(unscopedName("@my-org/notes-app")).toBe("notes-app");
  });

  it("passes an unscoped name through unchanged", () => {
    expect(unscopedName("click_counter")).toBe("click_counter");
  });
});

describe("slugify", () => {
  it("lowercases and collapses non-alphanumerics into single dashes", () => {
    expect(slugify("Click Counter")).toBe("click-counter");
    expect(slugify("click_counter")).toBe("click-counter");
  });

  it("trims leading and trailing dashes", () => {
    expect(slugify("--Click!!")).toBe("click");
  });

  it.each(["日本語アプリ", "★", "..."])("names a name that slugs to nothing: %j", (name) => {
    expect(slugify(name)).toBe("app");
  });
});

describe("defaultProductName", () => {
  it("derives from a package name, stripping its scope", () => {
    expect(defaultProductName("@my-org/notes-app")).toBe("notes-app");
  });

  it("returns undefined when there's no package name", () => {
    expect(defaultProductName(undefined)).toBeUndefined();
  });
});

describe("defaultIdentifier", () => {
  it("generates an org.inca.<slug> identifier from productName", () => {
    expect(defaultIdentifier("Click Counter")).toBe("org.inca.click-counter");
  });
});
