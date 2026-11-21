<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->
# inca-style

The CSS selector engine, the style sheet cascade, the inline style and the element spec over the
retained tree of `inca-node`.

Everything here is plain data and functions. A caller builds a tree, styles it and asks questions of
it.

```rust
use inca_node::VirtualTree;
use inca_style::{LengthSpec, StyleCache, build_spec, query_selector_all, set_style};

let mut tree = VirtualTree::new();
let root = tree.create_node("div")?;
let item = tree.create_node("p")?;
tree.set_root(root);
tree.append_child(root, item)?;
tree.set_attribute(item, "class", "note")?;
set_style(&mut tree, item, "width", 120.0)?;

// Every `.note` below `root`: just `item`.
assert_eq!(query_selector_all(&tree, root, ".note")?, [item]);
// The style of `item` in plain data: a width of 120 px.
let spec = build_spec(&tree, &StyleCache::default(), root).unwrap();
assert_eq!(spec.children[0].style.width, Some(LengthSpec::Px(120.0)));
```

## Selectors and the style sheet

A selector list is evaluated against a node and its subtree: the query and match operations of the
DOM, with a `SelectorError` that names the reason a selector does not parse.

The text of the `<style>` elements of the tree forms its style sheets. A sheet whose text is
unchanged keeps its parse. `StyleCache::computed_style` gives the style of a node after the rules
apply: matching declarations by specificity, then sheet order, then rule order, and the node's own
style last. `StyleCache::take_style_warnings` returns the warnings that sheet parses produce once.

## The element spec

`build_spec` reads a node and its subtree into an `ElementSpec`. Each node carries an element kind
and a style of lengths, colors, edges and keyword values, all plain data.

A style key may be `camelCase`, `kebab-case` or `snake_case`. `STYLE_KEYS` lists the keys the spec
reads. An unknown key and a malformed value are ignored, because nothing on the render path can
raise an exception, and `style_warning` reports such a key or value once, when it is set.

`wheel_axes` tells along which axes a wheel step can scroll a node and whether script can scroll it.

## Scope

Rendering, windows and JavaScript are outside this crate. A caller builds the tree through
`inca-node` and reads it back through the calls above.
