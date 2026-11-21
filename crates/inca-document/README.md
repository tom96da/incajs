<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->
# inca-document

What the elements of an `inca-node` tree mean beyond their attributes, and the geometry of boxes.

Everything here is plain data and functions.

## Semantics

`ARIA_REFLECTIONS` lists the `role` and `aria` properties that reflect on an element.
`AriaReferences` and `TabIndex` add to `VirtualTree` the reads and writes of the element references of an `aria`
attribute and the `tabindex` of an element, with the default of each element kind.

## Geometry

`Rect` and `Align` describe a box and where it lands in a client rectangle. `scroll_delta` gives the
distance that brings a box into view, and `scroll_chain` walks it up the scrolling ancestors.
