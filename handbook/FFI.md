<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Host bridge (FFI) reference

The function surface exposed to JS as `globalThis.__inca_native__`, bound
into the QuickJS context by the Rust host via `rquickjs`. JS and Rust run in
one process, and a call is an in-process function call with typed arguments.
No C ABI or IPC is involved.

## JS values stay in JS

Rust keeps ids and plain data. A handle kept past the call that produced it
outlives the context it belongs to, so every function and object lives on the
JS side. [Registering](#registering) is the implementation example: listener
functions stay in JS and Rust holds only the callback ids. Values the host
keeps for itself, such as the promise of a pending module and the event store,
sit in a per-realm vault (`inca_jsenv::vault`), a null-prototype object. A
read-only global function returns it only while Rust calls it through a native
gate, so no script reaches it, and Rust holds only the keys. The data of a
native class instance is the exception. An event object is an instance of a
native class. The JS values its fields hold, such as `target` and `detail`,
belong to that instance. So does the `searchParams` object of a `URL` object.
The garbage collector traces them, and they go when the event goes or, for
`searchParams`, with its `URL` object. They sit in one struct whose trace names
every field. A value left untraced makes the engine's drop abort the process.

All `unsafe` code of the Rust crates lives in the quickjs module of `inca-jsenv`
(`quickjs.rs` and `quickjs/`).
The one exception is the `unsafe impl` that rquickjs's `JsLifetime` derive
expands to, which gives a Rust value on a JS object its lifetime marker.
`clippy.toml` restricts that derive and `rquickjs::Persistent`, since one kept
past its runtime aborts the process when the runtime drops. The restriction is
lifted per type: at the item, or in a module of its own for a type with a
lifetime parameter. Don't lift it for a whole file with `#![allow]`. These
types derive it:

| Type | File | What it is | JS values it holds |
| --- | --- | --- | --- |
| `IncaEvent` | `inca-bridge/src/event/record.rs` | An event object | Through `Held`, traced |
| `Held` | `inca-bridge/src/event/record.rs` | The JS values of an event | `target`, `path`, `related` and `detail`, traced |
| `State` | `inca-bridge/src/event/record.rs` | The plain fields of an event | None |
| `UrlInstance` | `inca-jsenv/src/url.rs` | A `URL` object | Its `searchParams` object, traced |
| `ParamsInstance` | `inca-jsenv/src/url.rs` | A `URLSearchParams` object | None |
| `ParamsIterator` | `inca-jsenv/src/url/iterator.rs` | A `URLSearchParams` iterator | None |
| `Encoder` | `inca-jsenv/src/encoding/encoder.rs` | A `TextEncoder` object | None |
| `Decoder` | `inca-jsenv/src/encoding/decoder.rs` | A `TextDecoder` object | None |
| `SourceMaps` | `inca-jsenv/src/sourcemap/store.rs` | The source maps of a runtime | None |

## Retained virtual tree

The Rust host keeps an in-memory node structure (`VirtualNode`) that mirrors
the custom renderer's output. Each node has:

| Field | Type | Purpose |
| --- | --- | --- |
| `id` | `u32` | Stable handle returned to JS, used in all subsequent calls. |
| `tag_name` | `String` | Element kind, used to pick a GPUI element builder. |
| `style_props` | map | Layout/paint style properties. |
| `attributes` | map | Non-style attributes/props. |
| `parent` | `Option<u32>` | The node this one is attached to, if any. |
| `children` | `Vec<u32>` | Ordered child node handles. |

- A node has at most one parent. A `#fragment` has none.
- The text children of a connected `style` element hold rules. Layout and the
  scroll and focus checks read a node's `style_props` after the matching rules
  apply, and the `style_props` win over every rule. The host prints each
  dropped rule, at-rule and declaration through `warn` as `<style>: <line>`.
- `appendChild`, `insertBefore` and `replaceChild` detach the node first, so
  they also move a node. A `#fragment` moves its children in order and stays
  empty. Each call returns the position of the first inserted node among the
  children of `parentId`.
- Detaching, moving, replacing or destroying a node drops the focus its subtree
  holds at the call, with no `blur` or `focusout`. Detaching, moving or
  replacing a node sets the scroll offsets of its subtree to 0.
- `removeChild` and `replaceChild` keep the listeners of the node they detach.
- A `nodeId` that is not a finite whole number in the `u32` range throws a
  `TypeError`.
- `destroyNode` frees a node and its subtree at once. It frees nothing for the content of a
  template. At the start of each frame the host also frees every node that has no parent and is
  neither the root, held with `holdNode` nor the content of a template, with its subtree. The
  content of a template goes with the template.
- `releaseListeners` drops the listeners of a node, its subtree and the content of a template and
  leaves the nodes in the tree. A detached node stays readable until the host frees it.

### Tag vocabulary

`inca-node` classifies the tags in `ElementTag` (`spec.rs`) and lists the
tags that take focus by default in `focusable_by_default` (`tree.rs`).
`inca-gpui` builds the elements in `crates/inca-gpui/src/element/`. There are
seven kinds:

| `tag_name` | Maps to |
| --- | --- |
| `"#text"` | The tag core gives to text content. Content is its `"value"` string attribute (missing/non-string → empty, never a panic). Its children are not rendered. |
| `"button"` | A styled container. The host reads its `tabindex` with a default of 0. [Elements](../docs/reference/elements.md#button) covers its key clicks. |
| `"input"` | One line of editable text in a styled container. The host keeps its text; `inputValue` and `setInputValue` read and write it. Its children are not rendered. [Elements](../docs/reference/elements.md#input) covers the rest. |
| `"#comment"` | A generic styled container that marks a comment stand-in. Selector matching treats it as no element. |
| `"#fragment"` | A node that holds children and never has a parent. It is never connected, so layout, paint and focus skip it. Selector matching treats it as no element. The content of a template is a fragment that the template owns. |
| `"template"` | A node that is never laid out or painted, with its children. It owns a `#fragment` as its content, created with it. `templateContent` returns its id. The content has no parent and never counts as connected. The tag is matched ignoring ASCII case. |
| anything else | A generic styled container (a GPUI `div()`). |

### Style prop vocabulary

`inca-node` reads the keys in `spec/`, and `inca_node::STYLE_KEYS` lists every
key, its CSS names and the value it takes. `inca-gpui` maps the spec onto
`gpui` in `element/style.rs`. An unrecognized key, an invalid color and a value of a
shape the host does not support each print one warning line on stderr, once
for each node and warning line, and the value reads as unset. An empty string
`''` on a known key prints no warning. The table lists every key the host
reads. `set_style` and `remove_style` map the camelCase and kebab-case
spellings of a key to its snake_case form, and a CSS name to its host key, as
`inca_node::STYLE_KEYS` lists. All spellings of one prop share one slot. A key
with an uppercase initial, a mixed-case kebab key and a key that starts with
`-` are unknown.

A px value is a number or a string: a decimal with optional sign and exponent,
then `px` in any case (`"10px"`), or a string that reads as zero (`"0"`).
Surrounding whitespace is ignored. A decimal value is a number or a string
holding one decimal (`"0.5"`). Neither takes `%`. Each key keeps its range;
`opacity` clamps, any other value outside its range warns.

| Key | Value | Maps to |
| --- | --- | --- |
| `display` | `"flex"`\|`"block"`\|`"grid"`\|`"none"` | `Style::display` |
| `flex_direction` | `"row"`\|`"column"`\|`"row-reverse"`\|`"column-reverse"`\|`"row_reverse"`\|`"column_reverse"` | `flex_direction` |
| `justify_content` / `align_items` | `"start"`\|`"end"`\|`"flex-start"`\|`"flex-end"`\|`"center"`\|`"stretch"` | resp. fields |
| `gap` | px value (`>= 0`) | `gap.width` & `gap.height` (uniform) |
| `width` / `height` | px value (`>= 0`) or `"auto"` | `size.width` / `size.height` |
| `border_width` | px value (`>= 0`) | all four `border_widths.*` (uniform) |
| `background` / `border_color` / `text_color` | number from `0` to `0xffffff`, or string (`"#rgb"`, `"#rgba"`, `"#rrggbb"`, `"#rrggbbaa"`, `"transparent"`, `rgb()`, `rgba()`); any other value except `''` warns | `Fill`/`Hsla` |
| `corner_radius` | px value (`>= 0`) | all four `corner_radii.*` (uniform) |
| `text_size` | px value (`>= 0`) | `text.font_size` |
| `overflow` / `overflow_x` / `overflow_y` | `"visible"`\|`"hidden"`\|`"clip"`\|`"scroll"`\|`"auto"` (= `"scroll"`); `overflow` also takes `"<x> <y>"` | `overflow.x` / `overflow.y`; an axis key beats `overflow`; beside a `hidden` or `scroll` axis an unset or `visible` axis becomes `scroll` and `clip` becomes `hidden`; `hidden` clips and takes script scrolling, `clip` clips without scrolling, only `scroll` takes the wheel |
| `padding` (+ `_x`/`_y`/`_top`/`_right`/`_bottom`/`_left`) | px value (`>= 0`) | `padding.*`; side beats axis beats shorthand, a wrong-typed value falls through |
| `margin` (+ the same suffixes) | px value (negatives allowed) or `"auto"` | `margin.*`, same precedence |
| `flex_grow` / `flex_shrink` | decimal value `>= 0` | `flex_grow` / `flex_shrink` |
| `opacity` | decimal value, clamped to `0..=1` (`NaN` ignored) | `opacity` |
| `min_width` / `min_height` / `max_width` / `max_height` | px value (`>= 0`) or `"auto"` | `min_size.*` / `max_size.*` |

A color string ignores surrounding ASCII whitespace. `rgb()` and `rgba()` take
three channels and an optional alpha, in the comma syntax (all numbers or all
percentages) or the space syntax (`rgb(1 2 3 / 0.5)`, numbers and percentages
may mix). A channel and the alpha saturate at their limits and round half up.
The alpha has 256 steps.

`text_color` and `text_size` apply to containers. A text leaf takes them from
the nearest ancestor container.

A scroll container (`overflow` `hidden` or `scroll` on an axis) carries a
zero-size, absolutely placed child before its children, after the root's
pointer tracker.

## Binding functions

The functions live on `globalThis.__inca_native__`, a read-only,
non-configurable global. A parameter marked `?` may be left out, and
`undefined` or `null` there means its default. A parameter typed `| null` must
be passed, and `undefined` there counts as `null`.

| Function | Signature | Purpose |
| --- | --- | --- |
| `rootNodeId` | `() => number` | Return the id of the host-allocated root container, the node a mounting app attaches itself under. |
| `createNode` | `(tag: string) => number` | Allocate a `VirtualNode`, return its id. Throws once every id has been used. Freed at the next frame unless inserted or held, see Tree rules. |
| `appendChild` | `(parentId: number, nodeId: number) => number` | Attach a node at the end of `parentId`'s children. Same as `insertBefore` with no reference. Returns the position of the first inserted node. |
| `insertBefore` | `(parentId: number, nodeId: number, refId: number \| null) => number` | Attach a node before `refId`, or at the end if `null` or `undefined`. Returns the position of the first inserted node. |
| `removeChild` | `(parentId: number, childId: number) => void` | Detach a child node. Throws `NotFoundError` for a known node that is not a child. Freed at the next frame unless re-inserted or held, see Tree rules. |
| `replaceChild` | `(parentId: number, nodeId: number, childId: number) => number` | Put a node where `childId` was and detach `childId`. Returns the position of the first inserted node. |
| `linkedNode` | `(nodeId: number, link: "parent" \| "first" \| "last" \| "previous" \| "next") => number \| null` | The id of the parent, first child, last child, previous sibling or next sibling of `nodeId`, or `null` when there is none. Throws a `TypeError` for an unknown node or another `link`. |
| `childNodeIds` | `(nodeId: number) => number[]` | The ids of the children of `nodeId` in order. Throws a `TypeError` for an unknown node. |
| `isConnected` | `(nodeId: number) => boolean` | Whether `nodeId` is the root container or hangs under it. Throws a `TypeError` for an unknown node. |
| `cloneNode` | `(nodeId: number, deep: boolean) => [number, string][]` | Copy a node with its tag, attributes and style props, and its descendants when `deep` is truthy. A falsy `deep` clones only the node. Returns the id and tag of each copy in pre-order, the copy of `nodeId` first. The copy of a template is followed by the copy of its content (`"#fragment"`), the content descendants when `deep` is truthy, and then the other children. The copies have no listeners, focus or scroll offset, and the root copy has no parent. An `input` copy has the typed text of its original. Throws a `TypeError` for an unknown node. |
| `compareNodePosition` | `(nodeId: number, otherId: number) => number` | The position of `otherId` relative to `nodeId` as a bit mask: `1` disconnected, `2` preceding, `4` following, `8` contains, `16` contained by, `32` implementation specific. Equal ids give `0`. Nodes of different trees give `1 \| 32` plus `2` or `4`. Throws a `TypeError` for an unknown node. |
| `rootOf` | `(nodeId: number) => number` | The top of the parent chain of `nodeId`. A connected node gives the root container. Throws a `TypeError` for an unknown node. |
| `isEqualNode` | `(nodeId: number, otherId: number) => boolean` | Whether the two subtrees have the same tags, attributes, style props and children. The typed text of an `input` is not compared. Throws a `TypeError` for an unknown node. |
| `textContent` | `(nodeId: number) => string` | The data of a `#text` or `#comment` node. For any other node, the data of every descendant `#text` node in tree order. Throws a `TypeError` for an unknown node. |
| `wholeText` | `(nodeId: number) => string` | The data of `nodeId` and of the `#text` siblings that touch it, in order. Throws a `TypeError` for an unknown node. |
| `substringData` | `(nodeId: number, offset: number, count: number) => string` | The `count` UTF-16 code units of the data (the `"value"` attribute) from `offset`, as many as the data holds. Both arguments convert to unsigned 32-bit integers. A half of a surrogate pair that stands alone reads U+FFFD. Throws an `IndexSizeError` `DOMException` when `offset` is beyond the data and a `TypeError` for an unknown node. |
| `replaceData` | `(nodeId: number, offset: number, count: number, data: string) => void` | Replace `count` UTF-16 code units of the data from `offset` with `data`. Arguments and errors as `substringData`. |
| `splitText` | `(nodeId: number, offset: number) => number` | Cut the data at `offset`: `nodeId` keeps the head and a new `#text` node holds the tail. The new node follows `nodeId` when `nodeId` has a parent, and has none otherwise, so it is freed at the next frame unless held. Returns its id. Arguments and errors as `substringData`. |
| `holdNode` | `(nodeId: number) => void` | Keep `nodeId` and its subtree alive while it has no parent. |
| `releaseNode` | `(nodeId: number) => void` | End the hold. |
| `setAttribute` | `(nodeId: number, key: string, value: string \| number \| boolean) => void` | Set a non-style attribute prop. Names are folded to ASCII lower case. |
| `getAttribute` | `(nodeId: number, key: string) => string \| number \| boolean \| null` | Read a non-style attribute prop as stored, or `null` when the key is unset. Names are folded to ASCII lower case. |
| `removeAttribute` | `(nodeId: number, key: string) => void` | Remove a non-style attribute prop. Removing a key that isn't set does nothing. Names are folded to ASCII lower case. |
| `attributeNames` | `(nodeId: number) => string[]` | The keys of the non-style attributes of `nodeId`, in the order they were set. |
| `setStyle` | `(nodeId: number, key: string, value: string \| number \| boolean) => void` | Set a style prop. |
| `removeStyle` | `(nodeId: number, key: string) => void` | Remove a style prop so it renders as if never set. Removing a key that isn't set does nothing. |
| `inputValue` | `(nodeId: number) => string` | The text of the input node `nodeId`. Throws a `TypeError` for a node of another tag. |
| `setInputValue` | `(nodeId: number, value: string) => void` | Replace the text of the input node `nodeId`, without CR and LF. A changed text puts the caret at its end and ends a composition. Throws a `TypeError` for a node of another tag. |
| `addEventListener` | `(target: number \| "$document" \| "$window", event: string, callback: Function, capture?: boolean, passive?: boolean) => number` | Register `callback` for an event and return its id, see [Registering](#registering) and [Tagged targets](#tagged-targets). A `callback` that is not a function throws a `TypeError`. `capture` and `passive` default to `false`. |
| `removeEventListener` | `(target: number \| "$document" \| "$window", event: string, callbackId: number) => boolean` | Drop one registration and its table entry, and return whether it was there. An unknown node returns `false`. |
| `dispatchEvent` | `(target: number \| "$document" \| "$window", event: Event) => boolean` | Walk a script-built event over the path of `target` and return `false` when a listener canceled it, see [Dispatching from script](#dispatching-from-script) and [Tagged targets](#tagged-targets). |
| `click` | `(nodeId: number) => void` | Walk an untrusted `click` at `nodeId`, see [Dispatching from script](#dispatching-from-script). |
| `setPointerCapture` | `(nodeId: number, pointerId: number) => void` | Make `nodeId` the pending pointer capture while a button is held, see [Pointer events](#pointer-events). Throws `NotFoundError` for a `pointerId` other than 1 and `InvalidStateError` for a node outside the mounted tree or that names no node. |
| `releasePointerCapture` | `(nodeId: number, pointerId: number) => void` | Clear the pending capture when `nodeId` holds it. Throws `NotFoundError` for a `pointerId` other than 1. |
| `hasPointerCapture` | `(nodeId: number, pointerId: number) => boolean` | Whether `nodeId` is the pending capture and still mounted. `false` for a `pointerId` other than 1. |
| `templateContent` | `(nodeId: number) => number \| null` | The id of the content fragment of the template `nodeId`, the same id on every call. `null` for a node that is no template. Throws a `TypeError` for an unknown node. |
| `destroyNode` | `(nodeId: number) => number[]` | Free `nodeId` and its subtree, and the content of a template, release the table entries of every `callbackId` registered in it and return the ids. An unknown id returns `[]`. Destroying the root throws. Also drops the hold and the waiting `scrollIntoViewNode` calls of the destroyed nodes. |
| `releaseListeners` | `(nodeId: number) => number[]` | Drop every listener registered on `nodeId` and on the nodes below it, release their table entries and return their `callbackId`s. The nodes, their attributes and their children stay. An unknown id returns `[]`. |
| `matches` | `(nodeId: number, selector: string) => boolean` | Whether the element `nodeId` matches the selector list. Throws a `SyntaxError` for an invalid selector and a `TypeError` for an unknown node. A `#text` node and a `#comment` node match nothing. |
| `closest` | `(nodeId: number, selector: string) => number \| null` | The id of the nearest element at or above `nodeId` that matches. A `#text` node starts at its nearest element ancestor. Throws like `matches`. |
| `querySelector` | `(nodeId: number, selector: string) => number \| null` | The id of the first descendant of `nodeId` that matches the selector list, in document order. `nodeId` is never a result. `:scope` and a top-level `&` are `nodeId`, and the whole tree takes part in the match. Text and `#comment` nodes are never results, and a `#text` or `#comment` start finds nothing. Throws a `SyntaxError` for an invalid selector, also for a `#text` or `#comment` start, and a `TypeError` for an unknown node. |
| `querySelectorAll` | `(nodeId: number, selector: string) => number[]` | The ids of every descendant of `nodeId` that matches, each once, in document order. Follows the rules of `querySelector`. |
| `checkSelector` | `(selector: string) => void` | Throws a `SyntaxError` for an invalid selector list and returns for a valid one. It takes no node. |
| `focusNode` | `(nodeId: number, preventScroll?: boolean) => void` | Request focus for `nodeId`, next frame. Only a connected node with a `tabindex` that is not hidden takes it, and a `button` or `input` defaults to `0`. Unless `preventScroll` is `true`, the scroll containers above the node then scroll it into view: a node that shows stays put, a node outside a container's box centres, and a node that shows partly moves the least vertically and stays put horizontally. A node with no box yet is retried next frame. |
| `focusedNode` | `() => number \| null` | The id of the focused node, or `null` when none has focus. |
| `blurNode` | `(nodeId: number) => void` | Request that `nodeId` lose focus. Takes effect next frame, and does nothing if `nodeId` is not the focused node by then. |
| `layoutOf` | `(nodeId: number) => [x, y, width, height, border, scrollX, scrollY] \| null` | Read the border box of `nodeId` in window pixels as the last drawn frame placed it, with `border` the uniform border width. `scrollX` and `scrollY` are the positive scroll offset the parent applied to that box. `null` for a node with no recorded box. |
| `scrollOf` | `(nodeId: number) => [left, top, maxLeft, maxTop] \| null` | Read the scroll offset (positive) and its maximum as of the last drawn frame, with the offset clamped to that maximum. `null` for a node whose `overflow` is `visible` or `clip` on both axes. |
| `scrollIntoViewNode` | `(nodeId: number, block: string, inline: string) => void` | Scroll every scroll container above `nodeId` until the node shows, innermost first. `block` is the vertical and `inline` the horizontal alignment: `"start"`, `"center"`, `"end"` or `"nearest"`. `"nearest"` keeps a node that shows in place. A node shorter than the box aligns the edge that lies outside it, and a longer node aligns its end when its start lies before the box, and its start when its end lies after the box. Uses the boxes of the last drawn frame. A node with no recorded box is retried once on the next frame. |
| `scrollNode` | `(nodeId: number, left: number \| null, top: number \| null) => void` | Set the scroll offset of the axes given, a `null` axis keeps its value. Values below 0 become 0, and the host clamps the upper end at the next frame. A node whose `overflow` is `visible` or `clip` on both axes ignores the call. |

Shared throw rules, all as a `TypeError` except where noted:

- A malformed `nodeId` or `callbackId` throws, and so
  does a `target` string other than the two tags in
  [Tagged targets](#tagged-targets).
- A `capture`, `passive` or `preventScroll` that is not a boolean,
  `undefined` or `null` throws.
- `addEventListener` throws a `RangeError` after the last id.
- `setAttribute` and `setStyle` throw for a `value` that is not a string, number or boolean.
- `dispatchEvent` throws for an argument that is not an event of the host.
- A `nodeId`, `parentId`, `childId` or `refId` that names no node
  throws, except in the calls that let go of something: `releaseNode`,
  `destroyNode`, `releaseListeners`, `removeEventListener`, `blurNode` and
  the `childId` of `removeChild` ignore it. The host can free a node before
  the caller lets go of it.
- `scrollIntoViewNode` throws for a `block` or `inline` outside the four
  alignment names.

`dispatchEvent` also throws a `DOMException` named `InvalidStateError` for an
event that is being dispatched.

`appendChild`, `insertBefore` and `replaceChild` validate in this order, and
the first failing check throws. A failure changes nothing.

1. A `parentId`, `nodeId` or reference that names no node throws a `TypeError`.
2. A `#text` or `#comment` parent throws a `HierarchyRequestError`.
3. A `nodeId` that is `parentId` or one of its ancestors throws a
   `HierarchyRequestError`. The template that owns a content is an ancestor of
   the content and of every node in it.
4. A reference (`refId`, or `childId` of `replaceChild`) that is not a child of
   `parentId` throws a `NotFoundError`.
5. A `nodeId` naming the root, which belongs to the host, throws a
   `HierarchyRequestError`.

A `refId` equal to `nodeId` stands for the next sibling of `nodeId`.

The host converts the `VirtualNode` tree into GPUI elements on each frame.

### Event dispatch

The host dispatches native input to the JS callbacks registered for an event
target and event name. A walk is one dispatch of an event along its path.
Payloads and author-facing behaviour are in
[events.md](../docs/reference/events.md).

#### Registering

- `addEventListener` stores the function in the callback table of
  `inca-jsenv` and returns the table id. The ids come from the one allocator
  of the context, see [Timers](#timers), and run from 1 to `2^31 - 1`. The call
  after the last one throws a `RangeError`. Distinct ids stack on one
  `(target, event)`, and `capture` and `passive` belong to the registration.
  An id belongs to one registration, so `removeEventListener` takes the id
  only. `removeEventListener`, `destroyNode`, `releaseListeners` and the
  frame sweep release the table entries of the ids they drop. `destroyNode`
  and `releaseListeners` return the dropped ids because the caller still has
  to free the JS half of its own bookkeeping.
- `packages/core` keys a registration by target, event, capture and listener, so
  `removeEventListener(type, listener, options)` finds it by identity and
  adding the same listener again changes nothing. A listener object keeps its
  identity and receives `handleEvent` calls. A listener function runs with the
  event's `currentTarget` as `this`. `once` removes the registration before
  the listener runs. `createEventTarget` builds the listener methods for one
  target, a node or a tag.
- A walk reads each function from the table by id when it reaches the
  registration. An id with no table entry is skipped. A failed
  `addEventListener` (an unknown node, a value that is not a function or
  exhausted ids) stores nothing and takes no id.
- A throwing callback goes to `Host::failures`, and sibling callbacks still
  run. Unhandled promise rejections and failed jobs are reported through the
  job drain before listener errors, and only listener errors go to
  `Host::failures`. Failures are reported in order after each walk, after
  `drain_jobs_and_refresh` and by the development window's load poll
  (`EventDispatcher::report_pending`).

#### Released nodes

`__inca_released__(nodeIds: number[], callbackIds: number[]) => void` is a
global that core defines read-only on each load, so the latest copy of core
answers. Rust looks it up by name on each call and keeps nothing. A missing
global is skipped.

- Rust calls it at most once per frame, after the focus pass, and only when
  the sweep of that frame freed a node.
- `nodeIds` holds every freed node, subtrees and the content of a freed template included, and
  `callbackIds` every registration on them. Each id arrives once. Ids freed by
  `destroyNode` or dropped by `releaseListeners` never pass through it.
- A throw is reported to `Host::failures` and the frame goes on.

#### Event targets

`__inca_targets__(ids: (number | "$document" | "$window")[]) => unknown[]` is a
global that core defines read-only on each load, like `__inca_released__`, so
the latest copy of core answers. Rust looks it up by name on each walk and
keeps nothing.

- A walk that reaches a listener calls it once with the path, target first.
  Entry `i` of the result stands for `ids[i]` in `target`,
  `currentTarget` and `composedPath()`.
- An event whose payload names a related node calls it once more with that
  node, for `relatedTarget`.
- `dispatchEvent` calls it whether or not a listener hears the event.
- When the global is not a function, these fields carry the ids.
- A throw, or a result that is not an array, goes to `Host::failures` and the
  walk runs no listener. `dispatchEvent` throws it.
- Core gives the element handle of a node that has one, the `document` export
  for the `"$document"` tag, the `window` export for the `"$window"` tag, the
  element of the host root, which it creates on first use and `mount()`
  reuses, and `null` for any other node.

#### Tagged targets

The strings `"$document"` and `"$window"` name the `document` and
`window` exports in place of a node id in `addEventListener`, `removeEventListener`,
`dispatchEvent` and `__inca_targets__`. Rust holds them as `EventTargetId`
with the node ids.

- Any other string throws a `TypeError`.
- A registration on a tag takes `capture` and `passive` like one on a node,
  and `document` and `window` keep separate registrations of one event name.
- A path runs from the target node through its ancestors, the host root
  included, to `"$document"`, then `"$window"`. A node outside the root
  reaches neither global. A script dispatch at `"$document"` reaches
  `"$window"`, and one at `"$window"` stays there.
- A cut node silences both tags for that walk.
- Tags never reach `destroyNode`, the sweep or `__inca_released__`. The
  registrations live in the host and end with it, and the callbacks of the
  tags end with the JS realm.

#### Event object

- A walk passes one object to every listener on the path. The host creates
  it for its own events and a script builds it for the events it dispatches.
  Its fields are getters on the prototype of its kind, and its one own
  property is the getter `isTrusted`, shared by every event.
- `target` is the target the event is fired at, except where Pointer events and
  Keyboard and focus say otherwise. `currentTarget` is the target whose
  listeners run. `target`, `currentTarget`, the entries of `composedPath()`
  and `relatedTarget` carry what [Event targets](#event-targets) gives for
  those nodes. After the walk `currentTarget` is `null`, `eventPhase` is 0,
  `composedPath()` is empty and the stop flags are clear. `defaultPrevented`
  keeps its value.
- `eventId` grows with each event and is shared by all listeners of a walk.
  Only an event that reaches a listener enters the engine, drains jobs,
  requests a redraw and takes a number. One counter (`Host::last_event_id`)
  serves an engine's dispatchers and restarts at 1 with a new engine.
- `bubbles`, `cancelable` and `composed` come from one table (`EVENTS` in the
  `event_sink` module of `inca-gpui`) that also holds each event's movement rule and
  own-buttons rule. Other names get every rule `false`. The `timeStamp` origin is a
  process-wide `Instant` fixed when the first dispatcher is created.
- The object is an instance of the native class `IncaEvent`
  (`crates/inca-bridge/src/event/record.rs`), and its fields live in Rust.
  Events of every kind inherit from `Event.prototype`, which holds
  `stopPropagation`, `stopImmediatePropagation`, `preventDefault`,
  `composedPath`, `initEvent`, the phase constants (also on `Event`), the
  getters of the base fields, `returnValue` and `cancelBubble` (settable,
  ToBoolean), `srcElement` (reads `target`) and the non-enumerable `eventId`.
  A getter or a method called on any other receiver throws a `TypeError`. The
  prototypes sit in the realm's vault.
- Each kind of event inherits from a prototype of its kind that holds the
  getters of its payload fields. A UI prototype holds `view`, `detail` and
  `which`, and the focus, mouse and keyboard prototypes inherit from it.
  The pointer events, `click`, `auxclick` and `contextmenu` inherit from a
  pointer prototype and `wheel` from a wheel prototype, both children of the
  mouse one. `beforeinput` and `input` inherit from an input prototype and the
  composition events from a composition prototype, both children of the UI
  one. The mouse and keyboard prototypes hold their own `getModifierState`.
  A field getter or `getModifierState` throws a `TypeError` on an event of
  another kind.
  Scroll events and events built with `new Event` inherit from
  `Event.prototype`, events built with `new CustomEvent` from
  `CustomEvent.prototype`, which holds the `detail` getter, and events built
  with another constructor from the prototype of that class.
  - The walk turns the gpui payload into the payload of the event once, when
    it creates the event (`Payload::from_host`). `which` is `button + 1` on
    `mousedown`, `mouseup`, `click`, `dblclick`, `auxclick` and
    `contextmenu`, the `button` + 1 on every pointer event (0 for a `button`
    of -1), the lowest held button index + 1 on the other mouse events and 0
    with none held, 0 on `wheel`, `keyCode` on key events and 0 on
    focus events. `view` is `null` and `detail` is 0 on key and focus events.
    `keyCode` comes from `key` on a US layout (`event/key_code.rs`).
- GPUI's `platform` modifier becomes `metaKey` and `function` is dropped.
  `getModifierState` converts its argument to a string and reads those four
  fields.

#### Propagation and cancellation

- The capture pass runs the `capture` callbacks from `window` down to the
  target, with `eventPhase` 1 on ancestors and 2 on the target. The bubble
  pass runs the other callbacks from the target up while the event bubbles,
  with `eventPhase` 2 on the target and 3 on ancestors. A non-bubbling event
  runs the capture pass over the whole path and the bubble pass on the target
  only. At the target, capture callbacks run first.
- The path is fixed when the walk starts. Each pass reads a node's listeners
  when it reaches the node, so a listener removed earlier is skipped and one
  added during the node's callbacks skips that pass.
- `stopPropagation()` lets the remaining callbacks of that node in the
  current pass run and gives every later node and pass none. The target's
  capture and bubble callbacks are separate passes, so a stop in the first
  skips the second. `stopImmediatePropagation()` also stops the remaining
  callbacks of that node. Both leave gpui's own propagation running.
- A stop requested before a walk, through `stopPropagation()`,
  `stopImmediatePropagation()` or `cancelBubble = true`, gives that walk no
  callback. A dispatch that no listener hears clears it too.
- `preventDefault()` in a non-passive callback marks a cancelable event, and
  later listeners read `defaultPrevented` as `true`. `stopPropagation()` works
  in passive callbacks too.

#### Pointer events

- The target of `mousedown`, `mouseup`, `mousemove`, `wheel` and
  `contextmenu` is the deepest container under the pointer, or the root when
  there is none. While a pointer capture is active, the pointer events and
  `mousedown`, `mouseup`, `mousemove` go to the capturing node.
- While any listener is registered, any node can take focus or a style rule
  names `:hover` or `:active`, every non-inert container reports the pointer
  (`EventSink::tracks_pointer`) through a marker listener and one `on_hover`
  (`EventSink::hover_changed`), which keeps the hovered set. An inert node and
  its subtree stay outside hit testing and hover.
- The host builds a container with a scroll handle as `hidden`, so `gpui`
  never steps it, and applies one step itself after the `wheel` listeners ran.
  The step goes to the first container under the pointer that is live
  (connected, displayed, not `hidden`, not inert, `overflow` `scroll` on the
  axis) and can still move. It clamps at the end and the remainder goes
  nowhere. The root takes the step only when no container below it could move
  before the listeners. A container that scrolls on both axes takes the larger
  delta, and a tie moves along y.
- A canceled `wheel` applies no step and leaves offsets as the listeners left
  them. A script's write stays, and an uncanceled step adds to it.
- The mouse target slot is a gpui global in `inca-gpui`. The root's tracker
  clears it in the capture phase of each mouse event, and the first marker to
  run, the deepest container, keeps it. The tracker is the root's first child,
  so its bubble handlers run after every container below. It reports each
  input once (`EventSink::pointer_input`), and the click events fire after the
  `mouseup` input.
- `EventDispatcher::fire` and the pointer walks skip the callbacks of
  `mousedown`, `mouseup`, a mouse `click` and `dblclick` for a disabled
  button or input and its ancestors. Listeners below it run, and a key click
  always runs.
- A press record keeps the press container per button until a release of that
  button is reported. A press of another button leaves the record. Window
  deactivation clears every record (`inca_gpui::clear_presses`).
- `click` fires at the nearest common ancestor of the press container of that
  button and the release container, so a press must be recorded. A release
  over no container uses the root. The payload is `EventPayload::click`: the
  release position and modifiers, `button` the released button, `buttons` 0,
  `detail` the click count of the press, pointer source mouse. On macOS a left
  release with Control held completes no `click`.
- `dblclick`, `auxclick` and `contextmenu` fire through `EventSink::fire`.
  `dblclick` fires at the `click` ancestor right after the `click` whose press
  counts 2, with the pointer source cleared. `auxclick` fires there for the
  release of any non-left button, with the `click` payload. The root tracker
  fires `contextmenu` at the target of a right press, or on macOS a left press
  with Control, right after the `mousedown` input, with
  `EventPayload::context_menu`: `button` 2 and `buttons` 2 (`button` 0 and
  `buttons` 1 for Control), `detail` 0, pointer source mouse. It goes to the
  target of the press's `pointerdown`. On macOS gpui reports a Control+left
  press and its release as right ones without Control. While Control is held
  (`ModifiersChangedEvent` and pointer moves, the `ControlClick` global) the
  tracker turns them back into a left press and release with Control, and a
  left press with Control fires `contextmenu`.
- The first container to report in a pointer move calls
  `EventSink::pointer_over` with itself as the deepest and reports at once.
  Any other change reports once at the end of the update from the deepest
  container of the hovered set. Each report compares it with the previous
  deepest (`EventDispatcher::hover_prev`, `hover_path_difference`) and fires
  `mouseout` at the old one, `mouseleave` on each container left (innermost
  first), `mouseover` at the new one, `mouseenter` on each container entered
  (outermost first). The `mouseover`/`mouseout` walk starts at the deepest
  container. `relatedTarget` is the new container for `mouseout` and
  `mouseleave` and the old one for `mouseover` and `mouseenter`. A removed old
  container is skipped and its nearest remaining ancestor counts as the old
  one. Position and modifiers are read at the report. The pointer events
  `pointerout`, `pointerleave`, `pointerover` and `pointerenter` fire first, in
  the same order, and the mouse ones follow. While a capture is active the report
  takes the capturing node as the deepest container, and a gpui hover change
  only updates the hovered set. Each report also writes the path of the
  container under the pointer into the tree, and `:hover` rules read it.
- `EventSink::pointer_input` turns each raw mouse input into pointer events
  (`dispatch/pointer.rs`). The held-button mask in `Host::held_buttons` changes
  once per raw `mousedown` or `mouseup`. The first button down fires
  `pointerdown`, the last button up fires `pointerup`, and any other change
  fires `pointermove` with the changed `button`. Each fires before the mouse
  event of the input. `pointermove` fires before `mousemove` and shares its
  movement. A canceled `pointerdown` suppresses `mousedown`, `mousemove` and
  `mouseup` until the `pointerup`, and the press does not focus.
- Pointer capture keeps a pending target (`Host::capture_pending`) and an active
  one (`Host::capture_active`). The bindings change the pending target. The
  next pointer input, hover report or release makes it active: it fires
  `lostpointercapture` at the old target, `document` when that node left the
  tree, then `gotpointercapture` at the new one. Both copy the event that
  caused them. The final button release fires `pointerup`, `mouseup`,
  `lostpointercapture`, `click` or `auxclick` (at the `pointerup` target), then
  the boundary events back to the node under the pointer. Deactivating the
  window fires `lostpointercapture` at the active capture target, then clears
  the held buttons, the capture and a canceled `pointerdown`.

#### Mouse payload

- The payload is `clientX`, `clientY`, `screenX`, `screenY`, `offsetX`,
  `offsetY`, `x`, `y`, `pageX`, `pageY`, `movementX`, `movementY`, `button`,
  `buttons`, `detail`, `relatedTarget` (`null` except on hover events) and the
  four modifier flags.
  `x`/`y` and `pageX`/`pageY` equal `clientX`/`clientY`, and the sink sets
  `screenX`/`screenY` from `Window::bounds` plus the client position.
  The dispatcher sets `offsetX`/`offsetY` from the target's recorded box:
  the client position minus the box origin and border. A target with no
  record gets `clientX`/`clientY`.
- `MousePayload::pointer` adds `pointerId`, `pointerType`, `isPrimary`,
  `width`, `height` and `pressure`: a pointer event is 1, `"mouse"`, true, 1, 1,
  a mouse `click`, `auxclick` and `contextmenu` is 1, `"mouse"`, false, 1, 1,
  and the click of a key is -1, `""`, false, 1, 1. `pressure` is 0.5 on a
  pointer event when `buttons` is non-zero, else 0. `MousePayload::button` is
  `NO_BUTTON` for a pointer event about no button, which reads -1, and `detail`
  is 0 on every pointer event.
- `click`, `dblclick`, `auxclick` and `contextmenu` keep the `buttons` of
  their own payload. Other mouse events take it from `Host::held_buttons`,
  which every raw `mousedown` and `mouseup` updates.
- `movementX`/`movementY` come from raw pointer moves, recorded once per
  window. Only `mousemove` takes them, and every other event holds 0.

#### Keyboard and focus

- The target of `keydown`, `keypress` and `keyup` is the focused node, or the
  root when nothing is focused. The window focus then rests on
  `FocusRegistry::parked_handle`, which maps to no node.
- The root wires capture-phase `keydown` and `keyup` listeners whether or not
  a script listens. They call `EventSink::key_input`, which walks the event
  and then runs the default action of an event left uncanceled. `key` is
  `"Unidentified"` for an empty key, and a letter with Shift held is upper
  case when `key_char` is absent.
- `Tab` and `Shift+Tab` move focus, and Control, Alt and Meta cancel the move.
  `FocusRegistry::tab_move` collects the handled nodes in tree order and
  focuses the target, or blurs at the end of the order. While no node is
  focused, the move starts from the node of the last mouse press or the last
  focus (`FocusRegistry::press_start`). When that node leaves the tree the
  start moves to the node before its place.
- Enter clicks a focused enabled button on key down. Space clicks it on key
  up with any modifier. An Enter click, a Space key down, a Space key up or
  the button losing focus clears the pending Space. A button under a held
  mouse press also takes one Space click on key up. A click, the mouse
  release or the button losing focus ends the hold.
  A key click uses `MousePayload::keyboard_click`, and `preventDefault()` in
  `keydown`, the `keypress` of Enter, or `keyup` of the `button` ends it.
- The Menu key (`key_name` maps gpui's `"menu"` and the U+0010 of a Mac
  keyboard to `"ContextMenu"`) and Shift+F10 (not on macOS) fire
  `contextmenu` from `key_default` on `keydown`, unless a listener canceled
  it. It targets the focused node with `clientX`/`clientY` at the center of
  its recorded box, or the deepest recorded box at (1, 1) when nothing is
  focused. Its payload is mouse pointer source, `button` `NO_BUTTON` and
  `buttons` 0.
- A canceled `keydown`, `keypress` or `keyup` calls `Window::prevent_default`
  and `App::stop_propagation` after its walk. The platform then treats the key
  as handled, so later gpui key listeners stay idle. A stopped or unlistened key
  stays unhandled, so gpui key listeners still run. Keybindings run before key
  listeners, so a bound action consumes its key before any walk.
- `key_input` fires `keypress` after an uncanceled `keydown`, at the same
  target, when no Control or Meta is held, no composition runs, and the key is
  Enter without Alt or has a printable `key_char`. Alt may be held on macOS
  only, where Option composes a character. A canceled `keypress` cancels
  the key's default action, except for Space.
- The root wires `on_modifiers_changed` into `EventSink::modifiers_changed`.
  A modifier change bubbles `keydown` or `keyup` named `Shift`, `Control`,
  `Alt` or `Meta`, in that order, from the focused node or the root. It is
  diffed against `held_modifiers`, which `sync_modifiers` seeds from the
  window on the first frame and on every activation.
- `focusNode`/`blurNode` queue and apply in order on the next frame. A focus
  change dispatches `blur`, `focusout`, `focus`, `focusin` in that order, and
  the `focusin`/`focusout` walk has `target` the node. Changes to `tabindex`
  (key compared case-insensitively), `disabled` on a button or input,
  `hidden`, `inert` and `display`, and moving or removing a node, re-check
  the node's subtree before that frame's queued focus requests.
- An uncanceled `mousedown` focuses the nearest node on the target's path
  that has a `FocusRegistry` handle, and blurs the window when there is none.
  The tracker's capture handler calls `Window::prevent_default` to stop gpui's
  focus-on-press.
- A window activation change dispatches `blur` and `focusout`, or `focus` and
  `focusin`, to the focused node. While the window is inactive a later `blur`
  of that node is held back, and a node focused meanwhile receives `focus` and
  `focusin` on activation. `focus` also fires at `"$window"` before the
  focused node on activation, and `blur` after it on deactivation, whether or
  not a node is focused.

#### Text input

- The input element draws the text line inside the container. While the node
  is focused, each paint registers `TextInputHandler` with
  `Window::handle_input`, and the platform's text input and input method call
  it in UTF-16 code units. Reads answer from `Host::inputs`, writes go to
  `EventSink::input_request`.
- `inca_gpui::InputState` holds byte offsets on char boundaries. Each paint
  records the caret stops, the text origin and the sideways scroll
  (`EventSink::record_input`). Mouse hits, `bounds_for_range` and
  `character_index_for_point` read them. After each edit and selection change
  the host calls `Window::invalidate_character_coordinates`, so the platform
  moves the candidate window.
- The default action of an uncanceled `keydown` at a focused input with no
  composition edits it: the arrows, Home and End (Shift extends), Backspace
  by char and Delete by grapheme, and Secondary+A, C, X and V
  (`Modifiers::secondary`). ArrowUp and ArrowDown go to the start and the
  end. Such a key calls `Window::prevent_default` and `App::stop_propagation`.
  Typed characters arrive through the handler after the walk, without
  control characters.
- An uncanceled left `mousedown` on an input puts the caret at the nearest
  stop, and Shift extends the selection. On macOS a Ctrl+click keeps a
  selection it lands in and starts no drag. A move with the left button held
  extends it until the release. Tab into an input selects its text.
- `readonly` and `disabled` stop edits. A `readonly` input still moves its
  selection and copies. With a running composition it applies a selection
  only and leaves the composition open. A script that disables a composing
  input ends it with `compositionend`, then `blur`.
- An edit fires `beforeinput`, applies when no listener canceled it and the
  listeners left the value and the node, then fires `input`. A `beforeinput`
  of `insertCompositionText` is not cancelable (`flags_of`). Backspace or
  Delete with nothing to delete, typing into a `readonly` input and Enter
  fire `beforeinput` only. A paste's `beforeinput` carries the clipboard text
  as read and its `input` the inserted text.
- The first mark of a composition fires `compositionstart` with the selected
  text. Each mark fires `compositionupdate`, `beforeinput` and `input`. A
  commit fires the same, then `compositionend`. An empty mark or an unmark
  ends the composition with `compositionend`. Key events carry `isComposing`
  while the focused input composes.
- `begin_frame` hands each focus transition to the inputs before its events.
  A blurred input ends its composition, then fires `change` when the user
  edited it since its focus or its last `change` and its value differs from
  the value then. A focused input records its value. Enter and window
  deactivation fire `change` the same way and move the recorded value; on
  deactivation `change` comes before the node's `blur`. A script write is no
  edit and leaves the recorded value.

#### Dispatching from script

- The global constructors `Event`, `CustomEvent`, `UIEvent`, `FocusEvent`,
  `MouseEvent`, `PointerEvent`, `WheelEvent`, `KeyboardEvent`, `InputEvent` and
  `CompositionEvent` (`event/construct.rs`, `CLASSES`) build instances of the
  same class with the prototype of their kind, `isTrusted` false and `target`
  `null`. Each needs `new`, a type, and an `init` that is an object, `null` or
  `undefined`. Any other call throws a `TypeError`. Each constructor inherits
  from the constructor of the parent prototype, and `CustomEvent.prototype` from
  `Event.prototype`. The init members convert by their WebIDL type, and a
  non-finite coordinate, delta or size throws a `TypeError`.
- `initEvent()` and `initCustomEvent()` do nothing while the event is
  dispatched. Otherwise they set the type and the two flags, clear the stop
  and cancel flags and `isTrusted`, and keep `composed` and `target`.
- `dispatchEvent` runs the walk in the caller's context, so a listener can
  call it. It sets `target` and `isTrusted`, reads `type` and the flags
  (`bubbles`, `cancelable`, `composed`) from the event and returns the negated
  canceled flag. Default actions and the job queue belong to the caller. The
  walk writes no property of the event object, so a frozen event dispatches
  again.
- `click()` walks a `click` event with `MousePayload::keyboard_click`.
  `Host::clicking` holds the nodes whose click is running, as a re-entrancy
  guard. A disabled `button` or `input` returns at once.

- `HostedApp::render` fires `scroll` after the focus transitions and before
  building the tree. `Host::scrolled_nodes` compares each handle's clamped
  offset with the one recorded last and returns the changed nodes that a
  `scroll` listener reaches: a listener on the node itself, or a capture
  listener on one of its ancestors.

#### Event table

| Name | Payload |
| --- | --- |
| `click`, `auxclick`, `contextmenu` | mouse + pointer |
| `dblclick`, `mousedown`, `mouseup`, `mousemove` | mouse |
| `mouseenter`, `mouseleave`, `mouseover`, `mouseout` | mouse + `relatedTarget` |
| `pointerdown`, `pointerup`, `pointermove`, `gotpointercapture`, `lostpointercapture` | mouse + pointer |
| `pointerover`, `pointerout`, `pointerenter`, `pointerleave` | mouse + pointer + `relatedTarget` |
| `wheel` | mouse + `deltaX`, `deltaY`, `deltaZ`, `deltaMode` |
| `scroll` | none; every flag is false |
| `focus`, `blur`, `focusin`, `focusout` | `relatedTarget` |
| `keydown`, `keypress`, `keyup` | `key`, `repeat`, `isComposing`, modifier flags; `keypress` also `charCode`, `keyCode` and `which` of the typed character |
| `beforeinput`, `input` | `data`, `inputType`, `isComposing` |
| `change` | none |
| `compositionstart`, `compositionupdate`, `compositionend` | `data` |

`wheel` negates GPUI's `deltaX` and `deltaY`, `deltaZ` is 0 and `deltaMode` is
0 or 1.

### Timers

`inca_bridge::install` defines `setTimeout`, `setInterval`, `clearTimeout` and
`clearInterval` on `globalThis` through `inca_jsenv::timers::install`, so the
first line of the bundle sees them. The queue (`inca_jsenv::Timers`, owned by
`Host::timers`) keeps ids, due times and periods. The handler and its
arguments are an array in the callback table of `inca-jsenv`
(`inca_jsenv::callbacks`): a null-prototype object in the vault with one id
allocator for every registration of a context. Ids start at 1 and are never
reused. A script reaches neither the table nor the allocator.

- `setTimeout` and `setInterval` refuse a non-function handler with a
  `TypeError`, read `delay` as a 32-bit integer raised to 0 and throw the
  `RangeError` "timer ids are exhausted. restart the app" after the id
  `2^31 - 1`. The timer id is the table id.
- The clear functions are one function. They read the id as a 32-bit integer
  and release the table entry and the queue entry only for an id the queue
  holds, so the id of another kind of registration stays.
- A due timer calls its handler with `globalThis` as `this` and the stored
  arguments. A one-shot entry leaves the table before the handler runs.
- A timer started in a callback has the callback's nesting level plus 1. A
  timer of level above 6 whose delay is under 4 ms waits 4 ms. The level
  holds while the job queue drains after the callback. Rust keeps it: the
  turn records the level before it calls the handler, and the timer function
  reads it.
- A turn (`inca_jsenv::run_timers`) reads the clock once and runs each timer
  due at that reading in `(due, creation)` order. A repeating timer is queued
  again, at the first point of its schedule (the first due time plus whole
  periods) after that reading, before the handler runs. A timer started
  during the turn waits for the next turn.
- `EventDispatcher::start_timers` runs one foreground task per window. It
  waits on the wake channel and one executor timer, runs a turn when the
  earliest timer is due, and redraws only when a timer ran. `stop_timers`
  ends it. A reload stops the old task before it starts the new one.
- A throw from a handler goes to the error reporter. A listener error the
  callback caused goes to `Host::failures` and is reported after the turn.

### Frame callbacks

`inca_bridge::install` defines `requestAnimationFrame` and
`cancelAnimationFrame` on `globalThis` through `inca_jsenv::frames::install`.
The list (`inca_jsenv::Frames`, owned by `Host::frames`) keeps pairs of a
handle and a table id in registration order. The callback is in the callback
table, so a frame callback takes an id from the allocator of the timers and
listeners.

- `requestAnimationFrame` refuses a non-function with a `TypeError` and
  returns a handle. Handles come from the counter of `Frames`: from 1, step 1
  per request, the `RangeError` "timer ids are exhausted. restart the app"
  after `2^31 - 1`. A table that has no free id throws the same error.
- `cancelAnimationFrame` reads the handle as an unsigned 32-bit integer and
  throws a `TypeError` without an argument. It releases the table entry only
  for a handle `Frames` holds, so the id of a timer stays.
- `inca_jsenv::run_frames` moves the pending list into the batch of the frame
  and reads `performance.now()` once. Each callback of the batch receives that
  timestamp. The entry leaves the table before the callback runs, and a
  callback canceled by an earlier one of the batch does not run. The job
  queue drains after each callback. A callback requested during the run waits
  for the next frame.
- `EventDispatcher::start_timers` asks its window for a frame
  (`Window::on_next_frame`) while a callback is pending and no frame is
  queued. The frame runs the callbacks before the draw of that frame, drains
  the jobs and redraws when one ran, so a write of a callback shows in the
  same frame. No JS runs in a frame without a pending callback. A session
  without a window (`run_timers_idle`) runs no frame callbacks. The host sets
  `WindowOptions::inactive_frame_interval` to `None`, so a window without
  focus keeps its frame rate.
- `EventDispatcher::run_frame_callbacks` is the body of that frame callback.
  `Harness::settle` calls it once before its draw.

## Application menu

The host sets the menu bar (`crates/inca-host/src/menu.rs`). It holds one
menu, titled after the app, with one item, `Quit`, bound to `secondary-q`
(`cmd-q` on macOS, `ctrl-q` elsewhere). `Quit` belongs to the host, so every
app has a way to quit. Only macOS draws a menu bar today.

An item an app defines is dispatched at `window` as the event
`menu:<id>`, through `addEventListener` and the same registry. An item
`save` arrives as `menu:save`.
