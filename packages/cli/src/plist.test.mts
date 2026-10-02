// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, it } from "vitest";

import { encodePlist } from "./plist.mts";

describe("encodePlist", () => {
  it("escapes ampersands and angle brackets in string values", () => {
    const xml = encodePlist({ CFBundleName: `A&B <C> "D" 'E'` });

    expect(xml).toContain("<string>A&amp;B &lt;C&gt; \"D\" 'E'</string>");
  });

  it("escapes the ampersand first so an entity in the name is not left intact", () => {
    expect(encodePlist({ K: "&lt;" })).toContain("<string>&amp;lt;</string>");
  });

  it("leaves no raw angle bracket or ampersand from a value", () => {
    const xml = encodePlist({ K: "<&>" });
    const body = xml.slice(xml.indexOf("<string>"), xml.indexOf("</string>"));

    expect(body).toBe("<string>&lt;&amp;&gt;");
  });

  it("escapes keys the same way", () => {
    expect(encodePlist({ "a&<b>": "v" })).toContain("<key>a&amp;&lt;b&gt;</key>");
  });

  it("writes booleans as bare elements", () => {
    const xml = encodePlist({ On: true, Off: false });

    expect(xml).toContain("<key>On</key>\n\t<true/>");
    expect(xml).toContain("<key>Off</key>\n\t<false/>");
  });
});
