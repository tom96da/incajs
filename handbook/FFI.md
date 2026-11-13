<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Host bridge (FFI) reference

The function surface exposed to JS as `globalThis.__inca_native__`, bound into the QuickJS context
by the Rust host. JS and Rust run in one process, and a call is an in-process function call with
typed arguments. No C ABI or IPC is involved.

## JS values stay in JS

Rust keeps ids and plain data. A handle kept past the call that produced it outlives the context it
belongs to, so every function and object lives on the JS side. [Registering](#registering) is the
implementation example: listener functions stay in JS and Rust holds only the callback ids.

Values the host keeps for itself, such as the promise of a pending module and the event store, sit
in a per-realm vault, a null-prototype object. A read-only global function returns the vault only
while Rust calls it through a native gate, so no script reaches it. Rust holds only the keys.

The data of a native class instance is the exception. An event object is an instance of a native
class, and the JS values its fields hold, such as `target` and `detail`, belong to that instance. So
does the `searchParams` object of a `URL` object. The garbage collector traces them. They go when
the event goes or, for `searchParams`, with its `URL` object. Each class keeps them in one struct
whose trace names every field. A value left untraced makes the engine's drop abort the process.

All `unsafe` code of the Rust crates lives in the quickjs module of `inca-jsenv`. The one exception
is the `unsafe impl` that the `JsLifetime` derive expands to. It gives a Rust value on a JS object
its lifetime marker. These types derive it:

| Type | Crate | What it holds | JS values it holds |
| --- | --- | --- | --- |
| `IncaEvent` | `inca-bridge` | An event object | Through `Held`, traced |
| `Held` | `inca-bridge` | The JS values of an event | `target`, `path`, `related` and `detail`, traced |
| `State` | `inca-bridge` | The plain fields of an event | None |
| `TokenList` | `inca-bridge` | A `classList` object | None |
| `TokenListIterator` | `inca-bridge` | A `classList` iterator | None |
| `IncaStyle` | `inca-bridge` | A `CSSStyleDeclaration` object | None |
| `UrlInstance` | `inca-jsenv` | A `URL` object | Its `searchParams` object, traced |
| `ParamsInstance` | `inca-jsenv` | A `URLSearchParams` object | None |
| `ParamsIterator` | `inca-jsenv` | A `URLSearchParams` iterator | None |
| `PerformanceInstance` | `inca-jsenv` | The `performance` object | None |
| `EntryInstance` | `inca-jsenv` | A `PerformanceMark` or `PerformanceMeasure` object | None |
| `Encoder` | `inca-jsenv` | A `TextEncoder` object | None |
| `Decoder` | `inca-jsenv` | A `TextDecoder` object | None |
| `SourceMaps` | `inca-jsenv` | The source maps of a runtime | None |

## Retained virtual tree

The host keeps a retained tree of nodes. JS names each node by a `u32` id.

- A node has at most one parent. A `#fragment` has none.
- The text children of a connected `style` element hold rules. Layout and the scroll and focus
  checks read a node's style props after the matching rules apply, and the style props win over
  every rule. The host prints each dropped rule, at-rule and declaration through `warn` as `<style>:
  <line>`.
- `appendChild`, `insertBefore` and `replaceChild` detach the node first, so
  they also move a node. A `#fragment` moves its children in order and stays
  empty. Each call returns the position of the first inserted node among the
  children of `parentId`.
- Detaching, moving, replacing or destroying a node drops the focus its subtree
  holds at the call. Detaching, moving or replacing a node sets the scroll offsets of its subtree
  to 0.
- `removeChild` and `replaceChild` keep the listeners of the node they detach.
- `setInnerHtml` and `setOuterHtml` fire `blur` and then `focusout` at the focused node, before
  the tree changes, when it sits in the subtrees they remove. The other calls that drop focus
  fire nothing.
- A `nodeId` that is not a finite whole number in the `u32` range throws a
  `TypeError`.
- `destroyNode` frees a node and its subtree at once. It frees nothing for the content of a
  template. At the start of each frame the host also frees every node that has no parent and is
  neither the root, the document, held with `holdNode` nor the content of a template, with its
  subtree. The content of a template goes with the template. The root and the document stay,
  detached, with everything below them.
- `releaseListeners` drops the listeners of a node, its subtree and the content of a template and
  leaves the nodes in the tree. A detached node stays readable until the host frees it.

### Tag vocabulary

The host knows eight kinds of node:

| Tag | Maps to |
| --- | --- |
| `"#text"` | The tag core gives to text content. Content is its `"value"` string attribute (missing/non-string → empty, never a panic). Its children are not rendered. |
| `"button"` | A styled container. The host reads its `tabindex` with a default of 0. [Elements](../docs/reference/elements.md#button) covers its key clicks. |
| `"input"` | One line of editable text in a styled container. The host keeps its text; `inputValue` and `setInputValue` read and write it. Its children are not rendered. [Elements](../docs/reference/elements.md#input) covers the rest. |
| `"#document"` | The top of the tree, the parent of the root container. It has no parent and holds at most one element and any number of comments. It is never laid out, painted or matched. |
| `"#comment"` | A generic styled container that marks a comment stand-in. Selector matching treats it as no element. |
| `"#fragment"` | A node that holds children and never has a parent. It is never connected, so layout, paint and focus skip it. Selector matching treats it as no element. The content of a template is a fragment that the template owns. |
| `"template"` | A node that is never laid out or painted, with its children. It owns a `#fragment` as its content, created with it. `templateContent` returns its id. The content has no parent and never counts as connected. |
| anything else | A generic styled container. |

### Style prop vocabulary

An unrecognized key, an invalid color and a value of a shape the host does not support each print
one warning line on stderr, once for each node and warning line, and the value reads as unset. An
empty string `''` on a known key prints no warning. The table lists every key the host reads.
`setStyle` and `removeStyle` take the camelCase, kebab-case and snake_case spelling of a key and its
CSS name, and all share one slot. A key with an uppercase initial, a mixed-case kebab key and a key
that starts with `-` are unknown.

A px value is a number or a string: a decimal with optional sign and exponent,
then `px` in any case (`"10px"`), or a string that reads as zero (`"0"`).
Surrounding whitespace is ignored. A decimal value is a number or a string
holding one decimal (`"0.5"`). Neither takes `%`. Each key keeps its range;
`opacity` clamps, any other value outside its range warns.

The text that a style declaration reads back, through `getPropertyValue()`,
`cssText`, the `style` attribute and `outerHTML`, is computed from the stored
value:

- A px value or an `auto` value reads as its number in px, with `auto` in lower
  case.
- A number reads as its JS number text.
- An `opacity` reads as the number as written, without clamping.
- A keyword reads as its entry in the keyword list that matches it, ignoring
  ASCII case.
- A keyword pair reads as both keywords, or as one keyword when they are equal.
- A color reads as `transparent` for that keyword, otherwise as `rgb()`, and as
  `rgba()` when the alpha is below 255. The alpha is the shortest decimal that
  rounds back to the same byte.
- A value the reader rejects, and a boolean value, have no text and no
  declaration.
- The declarations read back in the order they were set.

| Key | Value | Rules |
| --- | --- | --- |
| `display` | `"flex"`\|`"block"`\|`"grid"`\|`"none"` | |
| `flex_direction` | `"row"`\|`"column"`\|`"row-reverse"`\|`"column-reverse"`\|`"row_reverse"`\|`"column_reverse"` | |
| `justify_content` / `align_items` | `"start"`\|`"end"`\|`"flex-start"`\|`"flex-end"`\|`"center"`\|`"stretch"` | |
| `gap` | px value (`>= 0`) | One gap for both axes. |
| `width` / `height` | px value (`>= 0`) or `"auto"` | |
| `border_width` | px value (`>= 0`) | One width for all four sides. |
| `background` / `border_color` / `text_color` | number from `0` to `0xffffff`, or string (`"#rgb"`, `"#rgba"`, `"#rrggbb"`, `"#rrggbbaa"`, `"transparent"`, `rgb()`, `rgba()`); any other value except `''` warns | |
| `corner_radius` | px value (`>= 0`) | One radius for all four corners. |
| `text_size` | px value (`>= 0`) | |
| `overflow` / `overflow_x` / `overflow_y` | `"visible"`\|`"hidden"`\|`"clip"`\|`"scroll"`\|`"auto"` (= `"scroll"`); `overflow` also takes `"<x> <y>"` | `overflow` sets both axes and an axis key beats it. Beside a `hidden` or `scroll` axis, an unset or `visible` axis becomes `scroll` and `clip` becomes `hidden`. `hidden` clips and takes script scrolling, `clip` clips without scrolling, and only `scroll` takes the wheel. |
| `padding` (+ `_x`/`_y`/`_top`/`_right`/`_bottom`/`_left`) | px value (`>= 0`) | `padding` sets the four sides, `_x` the left and right, `_y` the top and bottom. A side beats an axis and an axis beats `padding`. A value of the wrong type falls through to the next key. |
| `margin` (+ the same suffixes) | px value (negatives allowed) or `"auto"` | The sides and precedence of `padding`. |
| `flex_grow` / `flex_shrink` | decimal value `>= 0` | |
| `opacity` | decimal value, clamped to `0..=1` (`NaN` ignored) | |
| `min_width` / `min_height` / `max_width` / `max_height` | px value (`>= 0`) or `"auto"` | |

A color string ignores surrounding ASCII whitespace. `rgb()` and `rgba()` take
three channels and an optional alpha, in the comma syntax (all numbers or all
percentages) or the space syntax (`rgb(1 2 3 / 0.5)`, numbers and percentages
may mix). A channel and the alpha saturate at their limits and round half up.
The alpha has 256 steps.

`text_color` and `text_size` apply to containers. A text leaf takes them from
the nearest ancestor container.

## Binding functions

The functions live on `globalThis.__inca_native__`, a read-only,
non-configurable global. A parameter marked `?` may be left out, and
`undefined` or `null` there means its default. A parameter typed `| null` must
be passed, and `undefined` there counts as `null`.

| Function | Signature | Purpose |
| --- | --- | --- |
| `rootNodeId` | `() => number` | Return the id of the host-allocated root container, the node a mounting app attaches itself under. |
| `documentNodeId` | `() => number` | The id of the document node. |
| `createNode` | `(tag: string) => number` | Allocate a node, return its id. A tag that starts with `#` makes a node of that kind and any other tag makes an element of the HTML namespace. Throws once every id has been used. Freed at the next frame unless inserted or held, see [Retained virtual tree](#retained-virtual-tree). |
| `createElement` | `(localName: string) => [number, string]` | Allocate an element of the HTML namespace for `localName`, lowered in ASCII, and return its id and tag. Freed at the next frame unless inserted or held, see [Retained virtual tree](#retained-virtual-tree). Throws `InvalidCharacterError` for a name that is no valid element local name. |
| `createElementNS` | `(namespace: string \| null, qualifiedName: string) => [number, string, string \| null, string \| null, string]` | Allocate an element of `namespace` for `qualifiedName` and return its id, local name, namespace, prefix and tag name. An empty or `null` namespace gives no namespace. The name keeps its case. Freed at the next frame unless inserted or held, see [Retained virtual tree](#retained-virtual-tree). Throws `InvalidCharacterError` for a bad name and `NamespaceError` for a prefix the namespace does not allow. |
| `attributeKey` | `(nodeId: number, name: string) => string` | The key an attribute called `name` has on `nodeId`: ASCII lower case for an HTML element and `name` as given for any other node. Throws a `TypeError` for an unknown node. |
| `appendChild` | `(parentId: number, nodeId: number) => number` | Attach a node at the end of `parentId`'s children. Same as `insertBefore` with no reference. Returns the position of the first inserted node. |
| `insertBefore` | `(parentId: number, nodeId: number, refId: number \| null) => number` | Attach a node before `refId`, or at the end if `null` or `undefined`. Returns the position of the first inserted node. |
| `removeChild` | `(parentId: number, childId: number) => void` | Detach a child node. Throws `NotFoundError` for a known node that is not a child. Freed at the next frame unless re-inserted or held, see [Retained virtual tree](#retained-virtual-tree). |
| `replaceChild` | `(parentId: number, nodeId: number, childId: number) => number` | Put a node where `childId` was and detach `childId`. Returns the position of the first inserted node. |
| `linkedNode` | `(nodeId: number, link: "parent" \| "first" \| "last" \| "previous" \| "next") => number \| null` | The id of the parent, first child, last child, previous sibling or next sibling of `nodeId`, or `null` when there is none. Throws a `TypeError` for an unknown node or another `link`. |
| `childNodeIds` | `(nodeId: number) => number[]` | The ids of the children of `nodeId` in order. Throws a `TypeError` for an unknown node. |
| `isConnected` | `(nodeId: number) => boolean` | Whether the parent chain of `nodeId` reaches the document. The document is connected. Throws a `TypeError` for an unknown node. |
| `cloneNode` | `(nodeId: number, deep: boolean) => (number \| string \| null)[][]` | Copy a node with its name, attributes and style props, and its descendants when `deep` is truthy. A falsy `deep` clones only the node. Returns one entry per copy in pre-order, the copy of `nodeId` first. An element copy is `[id, localName, namespace \| null, prefix \| null, tagName]`. A `#text`, `#comment` or `#fragment` copy is `[id, tag]`. The copy of a template is followed by the copy of its content (`"#fragment"`), the content descendants when `deep` is truthy, and then the other children. The copies have no listeners, focus or scroll offset, and the root copy has no parent. An HTML `input` copy has the typed text of its original. Throws a `TypeError` for an unknown node. |
| `innerHtml` | `(nodeId: number) => string` | The markup of the children of `nodeId`. A template reads its content. Throws a `TypeError` for an unknown node. |
| `outerHtml` | `(nodeId: number) => string` | The markup of `nodeId` with its children. Throws a `TypeError` for an unknown node. |
| `setInnerHtml` | `(nodeId: number, html: string) => Parsed` | Parse `html` in the context of `nodeId` and replace all its children, or the children of its content for a template. The old children stay in the host as orphans with their listeners. Returns the parsed nodes at position 0. Throws a `TypeError` for an unknown node. |
| `setOuterHtml` | `(nodeId: number, html: string) => Parsed \| null` | Parse `html` in the context of the parent of `nodeId` and replace the node with the result. `null` and no change for a node without a parent. Throws `NoModificationAllowedError` for a node whose parent is the document and a `TypeError` for an unknown node. |
| `insertAdjacentHtml` | `(nodeId: number, position: string, html: string) => Parsed` | Parse `html` and insert the nodes at `position` (`beforebegin`, `afterbegin`, `beforeend` or `afterend`, matched ignoring ASCII case) of `nodeId`. The context is the container the nodes enter, and `body` for an `html` element. Throws `SyntaxError` for another position, `NoModificationAllowedError` for `beforebegin` or `afterend` on a node without a parent or a node whose parent is the document, and a `TypeError` for an unknown node. |
| `compareNodePosition` | `(nodeId: number, otherId: number) => number` | The position of `otherId` relative to `nodeId` as a bit mask: `1` disconnected, `2` preceding, `4` following, `8` contains, `16` contained by, `32` implementation specific. Equal ids give `0`. Nodes of different trees give `1 \| 32` plus `2` or `4`. Throws a `TypeError` for an unknown node. |
| `rootOf` | `(nodeId: number) => number` | The top of the parent chain of `nodeId`. A connected node gives the document. Throws a `TypeError` for an unknown node. |
| `isEqualNode` | `(nodeId: number, otherId: number) => boolean` | Whether the two subtrees have the same namespace, prefix, local name, attributes, style props and children. The typed text of an `input` is not compared. Throws a `TypeError` for an unknown node. |
| `textContent` | `(nodeId: number) => string` | The data of a `#text` or `#comment` node. For any other node, the data of every descendant `#text` node in tree order. Throws a `TypeError` for an unknown node. |
| `wholeText` | `(nodeId: number) => string` | The data of `nodeId` and of the `#text` siblings that touch it, in order. Throws a `TypeError` for an unknown node. |
| `substringData` | `(nodeId: number, offset: number, count: number) => string` | The `count` UTF-16 code units of the data (the `"value"` attribute) from `offset`, as many as the data holds. Both arguments convert to unsigned 32-bit integers. A half of a surrogate pair that stands alone reads U+FFFD. Throws an `IndexSizeError` `DOMException` when `offset` is beyond the data and a `TypeError` for an unknown node. |
| `replaceData` | `(nodeId: number, offset: number, count: number, data: string) => void` | Replace `count` UTF-16 code units of the data from `offset` with `data`. Arguments and errors as `substringData`. |
| `splitText` | `(nodeId: number, offset: number) => number` | Cut the data at `offset`: `nodeId` keeps the head and a new `#text` node holds the tail. The new node follows `nodeId` when `nodeId` has a parent, and has none otherwise, so it is freed at the next frame unless held. Returns its id. Arguments and errors as `substringData`. |
| `holdNode` | `(nodeId: number) => void` | Keep `nodeId` and its subtree alive while it has no parent. |
| `releaseNode` | `(nodeId: number) => void` | End the hold. |
| `setAttribute` | `(nodeId: number, key: string, value: string \| number \| boolean) => void` | Set an attribute prop. The `style` key applies the text as declarations, and the stored `style` text derives from the style props. An HTML element folds the name to ASCII lower case and any other node keeps it, as `attributeKey` gives. Throws `InvalidCharacterError` for a name that is empty or holds NUL, ASCII whitespace, `/`, `=` or `>`. |
| `getAttribute` | `(nodeId: number, key: string) => string \| number \| boolean \| null` | Read an attribute prop as stored, or `null` when the key is unset. The `style` text derives from the style props. The name folds as in `setAttribute`. |
| `removeAttribute` | `(nodeId: number, key: string) => void` | Remove an attribute prop. Removing `style` clears the style props. Removing a key that isn't set does nothing. The name folds as in `setAttribute`. |
| `attributeNames` | `(nodeId: number) => string[]` | The keys of the attributes of `nodeId`, in the order they were set. |
| `classList` | `(nodeId: number) => DOMTokenList` | The `DOMTokenList` over the `class` attribute of `nodeId`. |
| `setStyle` | `(nodeId: number, key: string, value: string \| number \| boolean) => void` | Set a style prop. The `style` attribute is derived again. |
| `removeStyle` | `(nodeId: number, key: string) => void` | Remove a style prop so it renders as if never set. Removing a key that isn't set does nothing. The `style` attribute is derived again. |
| `styleOf` | `(nodeId: number) => CSSStyleDeclaration` | Create a `CSSStyleDeclaration` object over the style props of `nodeId`. The element keeps one object for its lifetime. |
| `datasetGet` | `(nodeId: number, key: string) => string \| undefined` | The `data-*` attribute of `key` as text, or `undefined` when the attribute is absent or the key has no attribute name. Never throws for a key. |
| `datasetSet` | `(nodeId: number, key: string, value: string) => void` | Set the `data-*` attribute of `key`. Throws `SyntaxError` for a dash before a lower case letter in `key` and `InvalidCharacterError` for an attribute name that is not valid. |
| `datasetDelete` | `(nodeId: number, key: string) => boolean` | Remove the `data-*` attribute of `key`. Returns `true`, also when nothing was removed. |
| `datasetKeys` | `(nodeId: number) => string[]` | The keys of the `data-*` attributes of `nodeId` in attribute order. An attribute with an upper case letter in its name has no key and is left out. |
| `datasetTarget` | `() => object` | A new plain object whose prototype is `DOMStringMap.prototype`, the target of the `dataset` proxy. |
| `inputValue` | `(nodeId: number) => string` | The text of the input node `nodeId`. Throws a `TypeError` for a node of another tag. |
| `setInputValue` | `(nodeId: number, value: string) => void` | Replace the text of the input node `nodeId`, without CR and LF. A changed text puts the caret at its end and ends a composition. Throws a `TypeError` for a node of another tag. |
| `addEventListener` | `(target: number \| "$window", event: string, callback: Function, capture?: boolean, passive?: boolean) => number` | Register `callback` for an event and return its id, see [Registering](#registering) and [Tagged targets](#tagged-targets). A `callback` that is not a function throws a `TypeError`. `capture` and `passive` default to `false`. |
| `removeEventListener` | `(target: number \| "$window", event: string, callbackId: number) => boolean` | Drop one registration and its table entry, and return whether it was there. An unknown node returns `false`. |
| `dispatchEvent` | `(target: number \| "$window", event: Event) => boolean` | Walk a script-built event over the path of `target` and return `false` when a listener canceled it, see [Dispatching from script](#dispatching-from-script) and [Tagged targets](#tagged-targets). |
| `click` | `(nodeId: number) => void` | Walk an untrusted `click` at `nodeId`, see [Dispatching from script](#dispatching-from-script). |
| `setPointerCapture` | `(nodeId: number, pointerId: number) => void` | Make `nodeId` the pending pointer capture while a button is held, see [Pointer events](#pointer-events). Throws `NotFoundError` for a `pointerId` other than 1 and `InvalidStateError` for a node outside the mounted tree or that names no node. |
| `releasePointerCapture` | `(nodeId: number, pointerId: number) => void` | Clear the pending capture when `nodeId` holds it. Throws `NotFoundError` for a `pointerId` other than 1. |
| `hasPointerCapture` | `(nodeId: number, pointerId: number) => boolean` | Whether `nodeId` is the pending capture and still mounted. `false` for a `pointerId` other than 1. |
| `templateContent` | `(nodeId: number) => number \| null` | The id of the content fragment of the template `nodeId`, the same id on every call. `null` for a node that is no template. Throws a `TypeError` for an unknown node. |
| `destroyNode` | `(nodeId: number) => number[]` | Free `nodeId` and its subtree, and the content of a template, release the table entries of every `callbackId` registered in it and return the ids. An unknown id returns `[]`. Destroying the document, the root or a node above the root throws. Also drops the hold and the waiting `scrollIntoViewNode` calls of the destroyed nodes. |
| `releaseListeners` | `(nodeId: number) => number[]` | Drop every listener registered on `nodeId` and on the nodes below it, release their table entries and return their `callbackId`s. The nodes, their attributes and their children stay. An unknown id returns `[]`. |
| `matches` | `(nodeId: number, selector: string) => boolean` | Whether the element `nodeId` matches the selector list. Throws a `SyntaxError` for an invalid selector and a `TypeError` for an unknown node. A `#text` node and a `#comment` node match nothing. |
| `closest` | `(nodeId: number, selector: string) => number \| null` | The id of the nearest element at or above `nodeId` that matches. A `#text` node starts at its nearest element ancestor. Throws like `matches`. |
| `querySelector` | `(nodeId: number, selector: string) => number \| null` | The id of the first descendant of `nodeId` that matches the selector list, in document order. `nodeId` is never a result. `:scope` and a top-level `&` are `nodeId`, and the whole tree takes part in the match. The document has no scope element, so `:scope` selects the element of the document. Text and `#comment` nodes are never results, and a `#text` or `#comment` start finds nothing. Throws a `SyntaxError` for an invalid selector, also for a `#text` or `#comment` start, and a `TypeError` for an unknown node. |
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
  does a `target` string other than the tag in
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
- `classList`, `styleOf`, `datasetGet`, `datasetSet`, `datasetDelete` and `datasetKeys` do not throw
  for a freed `nodeId`. `classList` and `styleOf` return an object that reads empty, `datasetSet`
  still throws for a key that makes an invalid name and writes nothing, and the other calls read
  nothing.
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
5. A `nodeId` naming the document throws a `HierarchyRequestError`.
6. A document parent takes comments and at most one element, and throws a
   `HierarchyRequestError` for a text node, a fragment with text and a second
   element.

A `refId` equal to `nodeId` stands for the next sibling of `nodeId`.

### Parsed

`setInnerHtml`, `setOuterHtml` and `insertAdjacentHtml` return `[position, entries]`. `position` is
the index among the children of the container where the first inserted node sits, or would sit for
an empty result. Each entry is `[id, tag, namespace, parentId, data]` in pre-order, the content
fragments of templates included. `namespace` is the namespace of the node and `null` for a `#text`,
`#comment` or `#fragment` node. A parsed element has no prefix. An entry whose `parentId` is no id
of the list is a root. `data` is the text of a `#text` or `#comment` node and `null` for any other
node. A `style` attribute applies as style props and its warnings print through `warn`. An `input`
takes the typed text of its `value` attribute.

### Token lists

`classList` returns a `DOMTokenList` that holds the node id. The global `DOMTokenList` throws a
`TypeError` when called. The write methods throw a `DOMException` named `SyntaxError` for an empty
token and `InvalidCharacterError` for a token with ASCII whitespace. The host checks every token of
a call before it changes the `class` attribute. The iterators of `keys`, `values` and `entries` are
native class instances too. `forEach` reads the token count once, then reads each token from the
current list and skips a position the list no longer has, so a token that the callback adds is not
visited. The iterators read the live list at every step.

### Event dispatch

The host dispatches native input to the JS callbacks registered for an event
target and event name. A walk is one dispatch of an event along its path.
Payloads and author-facing behaviour are in
[events.md](../docs/reference/events.md).

#### Registering

- `addEventListener` stores the function in the callback table and returns the table id. The ids
  come from the one allocator of the context, see [Timers](#timers), and run from 1 to
  `2^31 - 1`. The call after the last one throws a `RangeError`. Distinct ids stack on one
  `(target, event)`, and `capture` and `passive` belong to the registration.
  An id belongs to one registration, so `removeEventListener` takes the id
  only. `removeEventListener`, `destroyNode`, `releaseListeners` and the
  frame sweep release the table entries of the ids they drop. `destroyNode`
  and `releaseListeners` return the dropped ids because the caller still has
  to free the JS half of its own bookkeeping.
- A walk reads each function from the table by id when it reaches the
  registration. An id with no table entry is skipped. A failed
  `addEventListener` (an unknown node, a value that is not a function or
  exhausted ids) stores nothing and takes no id.
- A throwing callback is reported, and sibling callbacks still run.
  [FAILURES.md](./FAILURES.md#where-a-failure-is-reported) gives when it is reported.

#### Released nodes

`__inca_released__(nodeIds: number[], callbackIds: number[]) => void` is a
global that core defines read-only on each load, so the latest copy of core
answers. The host looks it up by name on each call and keeps nothing. A missing
global is skipped.

Each frame runs the `requestAnimationFrame` callbacks before its draw. The draw then starts by
running, in order, the focus requests and their transitions, the sweep of nodes with no parent that
are neither the root, the document, held nor the content of a template, the `__inca_released__`
call, the focus reveal and the `scroll` events.

- The host calls `__inca_released__` at most once per frame, and only when the sweep of that frame
  freed a node.
- `nodeIds` holds every freed node, subtrees and the content of a freed template included, and
  `callbackIds` every registration on them. Each id arrives once. Ids freed by
  `destroyNode` or dropped by `releaseListeners` never pass through it.
- A throw is reported and the frame goes on.

#### Event targets

`__inca_targets__(ids: (number | "$window")[]) => unknown[]` is a
global that core defines read-only on each load, like `__inca_released__`, so
the latest copy of core answers. The host looks it up by name on each walk and
keeps nothing.

- A walk that reaches a listener calls it once with the path, target first.
  Entry `i` of the result stands for `ids[i]` in `target`,
  `currentTarget` and `composedPath()`.
- An event whose payload names a related node calls it once more with that
  node, for `relatedTarget`.
- `dispatchEvent` calls it whether or not a listener hears the event.
- When the global is not a function, these fields carry the ids.
- A throw, or a result that is not an array, is reported and the walk runs no listener.
  `dispatchEvent` throws it.
- Core gives the element handle of a node that has one, the `document` export
  for the document node, the `window` export for the `"$window"` tag, the
  element of the host root, which it creates on first use and `mount()`
  reuses, and `null` for any other node.

#### Tagged targets

The string `"$window"` names the `window` export in place of a node id in `addEventListener`,
`removeEventListener`, `dispatchEvent` and `__inca_targets__`.

- Any other string throws a `TypeError`.
- A registration on the tag takes `capture` and `passive` like one on a node,
  and `window` keeps its registrations apart from the nodes'.
- A path runs from the target node through its ancestors to the document,
  then `"$window"`. A node outside the document reaches no `"$window"`. A
  script dispatch at the document reaches `"$window"`, and one at `"$window"`
  stays there.
- A cut node silences `"$window"` for that walk.
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
- `eventId` grows with each event and is shared by all listeners of a walk. Only an event that
  reaches a listener enters the engine, drains jobs, requests a redraw and takes a number. One
  counter serves the whole engine and restarts at 1 with a new engine.
- `bubbles`, `cancelable` and `composed` are fixed per event name, and are `false` for a name the
  host does not fire. The `timeStamp` origin is fixed once per process.
- The object is an instance of a native class, and its fields live in Rust.
  Events of every kind inherit from `Event.prototype`, which holds
  `stopPropagation`, `stopImmediatePropagation`, `preventDefault`,
  `composedPath`, `initEvent`, the phase constants (also on `Event`), the
  getters of the base fields, `returnValue` and `cancelBubble` (settable,
  ToBoolean), `srcElement` (reads `target`) and the non-enumerable `eventId`.
  A getter or a method called on any other receiver throws a `TypeError`.
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
  - The host computes the payload once, when it creates the event. `which` is `button + 1` on
    `mousedown`, `mouseup`, `click`, `dblclick`, `auxclick` and
    `contextmenu`, the `button` + 1 on every pointer event (0 for a `button`
    of -1), the lowest held button index + 1 on the other mouse events and 0
    with none held, 0 on `wheel`, `keyCode` on key events and 0 on
    focus events. `view` is `null` and `detail` is 0 on key and focus events.
    `keyCode` comes from `key` on a US layout.
- The Command key on macOS and the Super key elsewhere set `metaKey`, and the Fn key sets no flag.
  `getModifierState` converts its argument to a string and reads `ctrlKey`, `shiftKey`, `altKey` and
  `metaKey` for `Control`, `Shift`, `Alt` and `Meta`. `Accel` reads `metaKey` on macOS and `ctrlKey`
  elsewhere. Any other name gives `false`.

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
  callbacks of that node.
- A stop requested before a walk, through `stopPropagation()`,
  `stopImmediatePropagation()` or `cancelBubble = true`, gives that walk no
  callback. A dispatch that no listener hears clears it too.
- `preventDefault()` in a non-passive callback marks a cancelable event, and
  later listeners read `defaultPrevented` as `true`. `stopPropagation()` works
  in passive callbacks too.

#### Pointer events

- The target of `mousedown`, `mouseup`, `mousemove`, `wheel` and
  `contextmenu` is the deepest container under the pointer, or the document
  element when there is none. While a pointer capture is active, the pointer events and
  `mousedown`, `mouseup`, `mousemove` go to the capturing node.
- An inert node and its subtree stay outside hit testing and hover.
- The host applies one scroll step for each `wheel` after its listeners ran.
  The step goes to the first container under the pointer that is live
  (connected, displayed, not `hidden`, not inert, `overflow` `scroll` on the
  axis) and can still move. It clamps at the end and the remainder goes
  nowhere. The root takes the step only when no container below it could move
  before the listeners. A container that scrolls on both axes takes the larger
  delta, and a tie moves along y.
- A canceled `wheel` applies no step and leaves offsets as the listeners left
  them. A script's write stays, and an uncanceled step adds to it.
- Each mouse input fires its events once, and the click events fire after the `mouseup`.
- The host skips the callbacks of `mousedown`, `mouseup`, a mouse `click` and `dblclick` for a
  disabled button or input and its ancestors. Listeners below it run, and a key click always runs.
- `click` fires at the nearest common ancestor of the container pressed with that button and the
  release container. A release with no press of its button fires no `click`, and a press of another
  button keeps the press. Window deactivation clears every press. A release over no container uses
  the document element. The payload carries the release position and modifiers, `button` the
  released button, `buttons` 0, `detail` the click count of the press, pointer source mouse. On
  macOS a left release with Control held completes no `click`.
- `dblclick` fires at the `click` ancestor right after the `click` whose press counts 2, with the
  pointer source cleared. `auxclick` fires there for the release of any non-left button, with the
  `click` payload. The host fires `contextmenu` at the target of a right press, or on macOS a left
  press with Control, right after the `mousedown` input, with `button` 2 and `buttons` 2 (`button` 0
  and `buttons` 1 for Control), `detail` 0, pointer source mouse. It goes to the target of the
  press's `pointerdown`. On macOS a Control+left press and its release arrive as a left press and
  release with Control.
- A pointer move reports the hover at once. Any other change of the containers under the pointer
  reports once, at the end of the update. Each report compares the deepest container under the
  pointer with the previous one and fires
  `mouseout` at the old one, `mouseleave` on each container left (innermost
  first), `mouseover` at the new one, `mouseenter` on each container entered
  (outermost first). The `mouseover`/`mouseout` walk starts at the deepest
  container. `relatedTarget` is the new container for `mouseout` and
  `mouseleave` and the old one for `mouseover` and `mouseenter`. The document is the
  outermost container of every path, so the reports reach it. A removed old
  container is skipped and its nearest remaining ancestor counts as the old
  one. A removed root fires no event, and only connected containers count as
  hovered. Position and modifiers are read at the report. The pointer events
  `pointerout`, `pointerleave`, `pointerover` and `pointerenter` fire first, in
  the same order, and the mouse ones follow. While a capture is active the report
  takes the capturing node as the deepest container, and a change of the containers under the
  pointer fires no event. Each report also writes the path of the
  container under the pointer into the tree, and `:hover` rules read it.
- Each mouse input fires pointer events. The held buttons change once per `mousedown` or `mouseup`
  input. The first button down fires
  `pointerdown`, the last button up fires `pointerup`, and any other change
  fires `pointermove` with the changed `button`. Each fires before the mouse
  event of the input. `pointermove` fires before `mousemove` and shares its
  movement. A canceled `pointerdown` suppresses `mousedown`, `mousemove` and
  `mouseup` until the `pointerup`, and the press does not focus.
- Pointer capture keeps a pending target and an active one. The bindings change the pending
  target. The
  next pointer input, hover report or release makes it active: it fires
  `lostpointercapture` at the old target, `document` when that node left the
  tree, then `gotpointercapture` at the new one. Both copy the event that
  caused them. The final button release fires `pointerup`, `mouseup`,
  `lostpointercapture`, `click` or `auxclick` (at the `pointerup` target), then
  the boundary events back to the node under the pointer. Deactivating the
  window fires `lostpointercapture` at the active capture target, then clears
  the held buttons, the capture and a canceled `pointerdown`.

#### Mouse payload

- The payload is `clientX`, `clientY`, `screenX`, `screenY`, `offsetX`, `offsetY`, `x`, `y`,
  `pageX`, `pageY`, `movementX`, `movementY`, `button`, `buttons`, `detail`, `relatedTarget` (`null`
  except on hover events) and the four modifier flags. `x`/`y` and `pageX`/`pageY` equal
  `clientX`/`clientY`, and `screenX`/`screenY` are the window origin plus the client position.
  `offsetX`/`offsetY` are the client position minus the origin and the border of the target's box in
  the last drawn frame. A target with no recorded box gets `clientX`/`clientY`.
- A pointer event adds `pointerId`, `pointerType`, `isPrimary`, `width`, `height` and `pressure`: a
  pointer event is 1, `"mouse"`, true, 1, 1, a mouse `click`, `auxclick` and `contextmenu` is 1,
  `"mouse"`, false, 1, 1, and the click of a key is -1, `""`, false, 1, 1. `pressure` is 0.5 on a
  pointer event when `buttons` is non-zero, else 0. `button` is -1 for a pointer event about no
  button, and `detail` is 0 on every pointer event.
- `click`, `dblclick`, `auxclick` and `contextmenu` keep the `buttons` of their own payload. Other
  mouse events take the held buttons, which every `mousedown` and `mouseup` input updates.
- `movementX`/`movementY` come from raw pointer moves, recorded once per
  window. Only `mousemove` takes them, and every other event holds 0.

#### Keyboard and focus

- The target of `keydown`, `keypress` and `keyup` is the focused node, or the element of the
  document when nothing is focused, else the document.
- Every `keydown` and `keyup` walks whether or not a script listens, and then runs the default
  action of an event left uncanceled. `key` is `"Unidentified"` for an empty key, and a letter with
  Shift held is upper case when the platform gives no typed character.
- `Tab` and `Shift+Tab` move focus, and Control, Alt and Meta cancel the move. The move focuses the
  next or previous node that can take focus in tree order, or blurs at the end of the order. While
  no node is focused, the move starts from the node of the last mouse press or the last focus. When
  that node leaves the tree the start moves to the node before its place.
- Enter clicks a focused enabled button on key down. Space clicks it on key
  up with any modifier. An Enter click, a Space key down, a Space key up or
  the button losing focus clears the pending Space. A button under a held
  mouse press also takes one Space click on key up. A click, the mouse
  release or the button losing focus ends the hold.
  A key click carries the payload of the click of a key, see [Mouse payload](#mouse-payload),
  and `preventDefault()` in `keydown`, the `keypress` of Enter, or `keyup` of the `button`
  ends it.
- The Menu key (`key` `"ContextMenu"`, also the U+0010 key of a Mac keyboard) and Shift+F10 (not on
  macOS) fire `contextmenu` on `keydown`, unless a listener canceled it. It targets the focused node
  with `clientX`/`clientY` at the center of its recorded box, or the deepest recorded box at (1, 1)
  when nothing is focused. Its payload is mouse pointer source, `button` -1 and `buttons` 0.
- A canceled `keydown`, `keypress` or `keyup` marks its key handled for the platform. A stopped or
  unlistened key stays unhandled. Keybindings run before the walk, so a bound action consumes its
  key before any walk.
- The host fires `keypress` after an uncanceled `keydown`, at the same
  target, when no Control or Meta is held, no composition runs, and the key is
  Enter without Alt or has a printable typed character. Alt may be held on macOS
  only, where Option composes a character. A canceled `keypress` cancels
  the key's default action, except for Space.
- A modifier change bubbles `keydown` or `keyup` named `Shift`, `Control`, `Alt` or `Meta`, in that
  order, from the focused node, else the document element, else the document. The host measures the
  change against the modifiers it reads from the window on the first frame and on every activation.
- `focusNode`/`blurNode` queue and apply in order on the next frame. A focus
  change dispatches `blur`, `focusout`, `focus`, `focusin` in that order, and
  the `focusin`/`focusout` walk has `target` the node. Changes to `tabindex`
  (key compared case-insensitively), `disabled` on a button or input,
  `hidden`, `inert` and `display`, and moving or removing a node, re-check
  the node's subtree before that frame's queued focus requests.
- An uncanceled `mousedown` focuses the nearest node on the target's path that can take focus, and
  blurs the window when there is none.
- A window activation change dispatches `blur` and `focusout`, or `focus` and
  `focusin`, to the focused node. While the window is inactive a later `blur`
  of that node is held back, and a node focused meanwhile receives `focus` and
  `focusin` on activation. `focus` also fires at `"$window"` before the
  focused node on activation, and `blur` after it on deactivation, whether or
  not a node is focused.

#### Text input

- While an input is focused, the platform's text input and input method read and edit it in UTF-16
  code units.
- The default action of an uncanceled `keydown` at a focused input with no composition edits it: the
  arrows, Home and End (Shift extends), Backspace by char and Delete by grapheme, and Secondary+A,
  C, X and V (Secondary is Command on macOS and Control elsewhere). ArrowUp and ArrowDown go to the
  start and the end. Typed characters arrive after the walk, without control characters.
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
  of `insertCompositionText` is not cancelable. Backspace or
  Delete with nothing to delete, typing into a `readonly` input and Enter
  fire `beforeinput` only. A paste's `beforeinput` carries the clipboard text
  as read and its `input` the inserted text.
- The first mark of a composition fires `compositionstart` with the selected
  text. Each mark fires `compositionupdate`, `beforeinput` and `input`. A
  commit fires the same, then `compositionend`. An empty mark or an unmark
  ends the composition with `compositionend`. Key events carry `isComposing`
  while the focused input composes.
- Each focus transition reaches the inputs before its focus events.
  A blurred input ends its composition, then fires `change` when the user
  edited it since its focus or its last `change` and its value differs from
  the value then. A focused input records its value. Enter and window
  deactivation fire `change` the same way and move the recorded value; on
  deactivation `change` comes before the node's `blur`. A script write is no
  edit and leaves the recorded value.

#### Scroll

- `scroll` fires at the start of the draw, in the order [Released nodes](#released-nodes) gives, at
  each container whose clamped offset changed since the last frame and that a `scroll` listener
  reaches: a listener on the node itself, or a capture listener on one of its ancestors. A script
  write and a wheel step each fire once.

#### Dispatching from script

- The global constructors `Event`, `CustomEvent`, `UIEvent`, `FocusEvent`,
  `MouseEvent`, `PointerEvent`, `WheelEvent`, `KeyboardEvent`, `InputEvent` and
  `CompositionEvent` build instances of the
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
- `click()` walks a `click` event with the payload of the click of a key. A `click()` at a node
  whose click is running returns at once, and so does one at a disabled `button` or `input`.

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

`wheel` negates the platform's `deltaX` and `deltaY`, `deltaZ` is 0 and `deltaMode` is 0 or 1.

### Timers

`setTimeout`, `setInterval`, `clearTimeout` and `clearInterval` are globals before the first line of
the bundle runs. The handler and its arguments sit in the callback table, which has one id allocator
for every registration of a context. Ids start at 1 and are never reused. A script reaches neither
the table nor the allocator.

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
  holds while the job queue drains after the callback.
- A turn reads the clock once and runs each timer
  due at that reading in `(due, creation)` order. A repeating timer is queued
  again, at the first point of its schedule (the first due time plus whole
  periods) after that reading, before the handler runs. A timer started
  during the turn waits for the next turn.
- A reload stops the timers of the old bundle when the new bundle takes the window, so no old timer
  fires after that.
- A throw from a handler is reported, see [FAILURES.md](./FAILURES.md#where-a-failure-is-reported).

### Frame callbacks

`requestAnimationFrame` and `cancelAnimationFrame` are globals like the timers. The pending
callbacks keep their registration order. A callback sits in the callback table, so a frame callback
takes an id from the allocator of the timers and listeners.

- `requestAnimationFrame` refuses a non-function with a `TypeError` and
  returns a handle. Handles come from a counter of their own: from 1, step 1
  per request, the `RangeError` "timer ids are exhausted. restart the app"
  after `2^31 - 1`. A table that has no free id throws the same error.
- `cancelAnimationFrame` reads the handle as an unsigned 32-bit integer and
  throws a `TypeError` without an argument. It releases the table entry only
  for a pending handle, so the id of a timer stays.
- A frame moves the pending callbacks into its batch and reads `performance.now()` once. Each
  callback of the batch receives that timestamp. The entry leaves the table before the callback
  runs, and a callback canceled by an earlier one of the batch does not run. The job queue drains
  after each callback. A callback requested during the run waits for the next frame.
- The frame runs the callbacks before its draw, drains the jobs and redraws when one ran, so a write
  of a callback shows in the same frame. No JS runs in a frame without a pending callback. A session
  without a window runs no frame callbacks. A window without focus keeps its frame rate.
