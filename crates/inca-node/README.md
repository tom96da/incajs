<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->
# inca-node

The retained virtual tree (`VirtualTree`), the CSS selector engine, the style sheet cascade, the
element spec and the pure logic that goes with them: wheel axes, scroll-into-view geometry and the
filters for typed and pasted text.

Everything here is plain data and functions. A caller builds a tree, mutates it and asks questions
of it.

```rust
use inca_node::{LengthSpec, VirtualTree, build_spec, query_selector_all};

let mut tree = VirtualTree::new();
let root = tree.create_node("div")?;
let item = tree.create_node("p")?;
tree.set_root(root);
tree.append_child(root, item)?;
tree.set_attribute(item, "class", "note")?;
tree.set_style(item, "width", 120.0)?;

// Every `.note` below `root`: just `item`.
assert_eq!(query_selector_all(&tree, root, ".note")?, [item]);
// The style of `item` in plain data: a width of 120 px.
let spec = build_spec(&tree, root).unwrap();
assert_eq!(spec.children[0].style.width, Some(LengthSpec::Px(120.0)));
```

## The tree

A node has a tag name, attributes kept in insertion order, style props and child links. A node has
one parent: attaching it to a new parent detaches it from the old one. The tag `"#text"` makes a
text node.

A parentless node that is neither the root, held nor the content of a template is an orphan.
`VirtualTree::sweep` frees every orphan with its subtree, and a held node stays out of the sweep. A
node can also be destroyed on its own, and a subtree can be cloned.

A `template` element owns a content fragment. The fragment has no parent, and it goes with the
template.

Markup parses into the tree as the children of a context node.

The tree answers the node relations and the text operations of the DOM: the ancestors and
siblings of a node, the position of one node against another, and the reading and editing of
text data. It also records the pointer and focus state that `:hover`, `:active`, `:focus` and
`:focus-within` match.

## Selectors and the style sheet

A selector list is evaluated against a node and its subtree: the query and match operations of the
DOM, with a `SelectorError` that names the reason a selector does not parse.

The text of the `<style>` elements of the tree forms its style sheets. A sheet whose text is
unchanged keeps its parse. `VirtualTree::computed_style` gives the style of a node after the rules
apply: matching declarations by specificity, then sheet order, then rule order, and the node's own
style last. The warnings that sheet parses produce are returned once.

## The element spec

`build_spec` reads a node and its subtree into an `ElementSpec`. Each node carries an element kind
and a style of lengths, colors, edges and keyword values, all plain data.

A style key may be `camelCase`, `kebab-case` or `snake_case`. `STYLE_KEYS` lists the keys the spec
reads. An unknown key and a malformed value are ignored, because nothing on the render path can
raise an exception, and `style_warning` reports such a key or value once, when it is set.

## Scroll geometry

The spec tells along which axes a wheel step can scroll a node and whether script can scroll it.
Scrolling a `Rect` into view in a visible `Rect` gives a scroll change per axis, and a chain of
containers applies it from the innermost outward.

## Text filters

Filters for typed and pasted text drop control characters. A pasted line break becomes one space.

## Scope

Rendering, windows and JavaScript are outside this crate. A caller builds the tree through the calls
above and reads it back.
