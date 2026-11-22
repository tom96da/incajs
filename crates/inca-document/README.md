<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->
# inca-document

What the elements of an `inca-node` tree mean beyond their attributes, how a document is built and
changed, the state of a document, the event model, the focus model and the geometry of boxes.

Everything here is plain data and functions.

## Semantics

`ARIA_REFLECTIONS` lists the `role` and `aria` properties that reflect on an element.
`AriaReferences` and `TabIndex` add to `VirtualTree` the reads and writes of the element references
of an `aria` attribute and the `tabindex` of an element, with the default of each element kind.

## Focus

`FocusModel` holds the node that has focus, the focus and blur requests that wait for the next
frame, the nodes whose `tabindex` changed and where Tab starts, and it moves focus along the Tab
order. A `FocusSurface` is the window side that creates the handle of a node and moves the window's
focus.

## Document

`new_document_tree` builds the tree of a document. `validate_and_extract` and
`is_element_local_name` check element names. `store_attribute` writes an attribute and applies a
`style` attribute as style props, and warnings go to a `StyleWarnings` sink. `insert_parsed`,
`replace_children` and `replace_with` are the markup setters, and `apply_removals` carries the focus
steps of a tree call that moved or removed nodes. The `text` module holds the text-control model.
`DocumentState` holds the tree with its cascade, the listeners, the signals and the focus, and
`StyleWarnings` collects the warnings of a style write.

## Events

`EVENTS` and `Flags` describe each event type. `EventPayload`, `Modifiers` and the other plain
payloads carry what an event reports. `EventListeners`, `Signals` and `Links` hold the listeners and
the abort signals and links of a document. `EventTargetId` names a target, `Walk` describes one
dispatch and `deliver` runs it through an `Invoke`. `key_code` and `keypress_code` give the `code`
of a key.

## View

`View` is the window as the document sees it: the boxes of the last drawn frame, the scroll
positions, the held mouse buttons and the reveals that wait for a box.

## Geometry

`Rect` and `Align` describe a box and where it lands in a client rectangle. `scroll_delta` gives the
distance that brings a box into view, and `scroll_chain` walks it up the scrolling ancestors.
