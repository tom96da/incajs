<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->
# inca-node

The retained virtual tree (`VirtualTree`) and the pure logic that goes with it: the semantics of the
elements, scroll-into-view geometry and the filters for typed and pasted text.

Everything here is plain data and functions. A caller builds a tree, mutates it and asks questions
of it.

```rust
use inca_node::VirtualTree;

let mut tree = VirtualTree::new();
let root = tree.create_node("div")?;
let item = tree.create_node("p")?;
tree.set_root(root);
tree.append_child(root, item)?;
tree.set_attribute(item, "class", "note")?;

assert_eq!(tree.get(item).unwrap().parent(), Some(root));
```

## The tree

A node has a tag name, attributes kept in insertion order, style props and child links. A node has
one parent: attaching it to a new parent detaches it from the old one. The tag `"#text"` makes a
text node.

A parentless node that is neither the root, the document, held nor the content of a template is an
orphan. `VirtualTree::sweep` frees every orphan with its subtree, and a held node stays out of the
sweep. A node can also be destroyed on its own, and a subtree can be cloned.

A `template` element owns a content fragment. The fragment has no parent, and it goes with the
template.

Markup parses into the tree as the children of a context node. A node, or the children of a node,
writes back as markup.

The tree answers the node relations and the text operations of the DOM: the ancestors and
siblings of a node, the position of one node against another, and the reading and editing of
text data. It also records the pointer and focus state that `:hover`, `:active`, `:focus` and
`:focus-within` match.

## Scroll geometry

Scrolling a `Rect` into view in a visible `Rect` gives a scroll change per axis, and a chain of
containers applies it from the innermost outward.

## Text filters

Filters for typed and pasted text drop control characters. A pasted line break becomes one space.

## Scope

Rendering, windows and JavaScript are outside this crate. A caller builds the tree through the calls
above and reads it back.
