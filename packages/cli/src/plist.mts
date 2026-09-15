// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/** A macOS `Info.plist`'s value types — the only ones `inca package` writes. */
export type PlistValue = string | boolean;

function escapeXml(value: string): string {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

function encodeValue(value: PlistValue): string {
  if (typeof value === "boolean") return value ? "<true/>" : "<false/>";
  return `<string>${escapeXml(value)}</string>`;
}

/** Encodes a flat key/value dict as an `Info.plist` XML document. */
export function encodePlist(dict: Record<string, PlistValue>): string {
  const entries = Object.entries(dict)
    .map(([key, value]) => `\t<key>${escapeXml(key)}</key>\n\t${encodeValue(value)}`)
    .join("\n");

  return (
    '<?xml version="1.0" encoding="UTF-8"?>\n' +
    '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n' +
    '<plist version="1.0">\n' +
    "<dict>\n" +
    `${entries}\n` +
    "</dict>\n</plist>\n"
  );
}
