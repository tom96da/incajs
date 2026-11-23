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

`FocusModel` holds the nodes that take focus, the node that has focus, the focus and blur requests
that wait for the next frame, the nodes whose `tabindex` changed and where Tab starts. Each frame
and each Tab press it returns a `FocusPlan`: the handles to create and drop, the moves of the
window's focus and the transitions to report. `RevealQueue` holds the reveals that wait for a box,
from a focus and from `scrollIntoView`.

## Document

`new_document_tree` builds the tree of a document. `validate_and_extract` and
`is_element_local_name` check element names. `insert_parsed`, `replace_children` and `replace_with`
are the markup setters. `parse_inner`, `parse_outer` and `parse_adjacent` parse the markup of
`innerHTML`, `outerHTML` and `insertAdjacentHTML` and apply each `style` attribute as style props.
The `text` module holds the text-control model.
`DocumentState` holds the tree with its cascade, the listeners, the signals, the focus and the
reveals. The tree is read through `DocumentState::tree` and changed only through the operations of
the document, each of which runs the steps its change needs: a node that moves or leaves drops the
focus it held with the reveals a focus asked for, moves the Tab start, queues a focus re-check and
resets the scroll offsets of its subtree in a `View`; a freed node drops its state in the `View` and
in the document; an attribute or style write queues the focus re-check it needs and reports its
warnings to a `StyleWarnings` sink; a copy of an `input` takes its typed text. The document changes
the focus only through its own methods. The tree holds the drawn focus, which the document publishes
at each frame and at each change of the window's activation. A script query matches `:focus` against
the focus once the queued requests apply.

## Events

`EVENTS` and `Flags` describe each event type. `EventPayload`, `Modifiers` and the other plain
payloads carry what an event reports. `EventListeners`, `Signals` and `Links` hold the listeners and
the abort signals and links of a document. `EventTargetId` names a target, `Walk` describes one
dispatch and `deliver` runs it through an `Invoke`. `key_code` and `keypress_code` give the `code`
of a key.

## View

`View` is the window as the document sees it: the boxes of the last drawn frame, the scroll
positions and the held mouse buttons.

## Geometry

`Rect` and `Align` describe a box and where it lands in a client rectangle. `scroll_delta` gives the
distance that brings a box into view, and `scroll_chain` walks it up the scrolling ancestors.
`Align::from_name` reads the alignment a scroll call names. `client_width` and `client_height` give
the client size of a box, and `offset_of` gives where a box sits inside the padding edge of its
parent.
