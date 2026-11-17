// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Document } from "../types.mts";
import type { Comment, DocumentFragment, Element, Text } from "./types.mts";

/** Any node handle. Code that reads the internal `kind` takes this union and narrows by it. */
export type HandleNode = Element | Comment | Text | DocumentFragment | Document;
