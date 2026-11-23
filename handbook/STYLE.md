<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Style

How code, comments and documentation are written in this repository.

## Roles

Each place that carries text has one role.

- **Code** says How.
- **Tests** say What. Test names and comments state the behaviour they pin.
- **Commit messages** say Why.
- **`//` comments** say Why not. One line states the constraint that rules out the obvious
  alternative.

## Enforcement

A convention that a tool can check is enforced by one of three tools.

- **`oxlint`** checks the code against rules, configured in
  [`oxlint.config.ts`](../oxlint.config.ts).
- **`oxfmt`** formats the code, configured in
  [`oxfmt.config.ts`](../oxfmt.config.ts).
- **`clippy`** checks the Rust code against lints, configured in
  [`clippy.toml`](../clippy.toml).

A convention without a rule is a review criterion.

## Restricted Rust types

`clippy.toml` restricts each item (a type, a macro or a derive) whose use can let a value outlive
the JS runtime that owns what it refers to, for example `rquickjs::Persistent` and `JsLifetime`. A
value kept past its runtime aborts the process when the runtime drops.

- **Scope**: lift the restriction per item, with an `allow` at the use. A type with a lifetime
  parameter sits in a module of its own, and the `allow` goes on that module. Don't lift it for a
  whole file with `#![allow]`.
- **Reason**: each lifting carries a one-line comment with the reason.

## Prose

Comments, documentation and commit messages state the facts that hold.

- **Plain wording**: write the fact directly. Don't add a framing clause that softens or decorates
  it.
- **Present behaviour**: describe what the code does. Don't write counterfactuals or the history of
  earlier versions.
- **Positive statements**: write what holds. Don't write "not X but Y", "instead of X" or "rather
  than X".

## Shape of a module

- A parent module declares and re-exports its children and holds no logic. A Rust parent is a `mod`
  with child modules, and a TypeScript parent is an `index.mts`.
- A file holds one concern.
- A concept lives in one file. Before adding an export, look for a file with the same purpose.
- A file stays at or below 800 lines, and a merge never makes it longer.

### Rust

- A module path is at most two levels below `src/`.
- A file under about 200 lines as a whole, tests included, reads as small, and a merge with a
  neighbour of the same concern is considered. A seam or a file of its own concern may stay small.

### TypeScript

- A function lives in its own file, and `index.mts` only re-exports.

## Public API docs

rustdoc and TSDoc on a public item are reference material for the framework user.

- **Reference**: describe what the item does, its contract and its errors, and when to pass an
  option. Don't explain how it is implemented.
- **Parameters and results**: document each parameter, each return value and each error of a public
  function with the tags of the language. TSDoc uses `@param name - description`, `@returns` and
  `@throws`. rustdoc has no tag for a parameter or a return value, so the description states them,
  and it uses the sections `# Errors` and `# Panics`. Don't restate what the name already says.
- **Types in signatures**: a type that a public signature names is documented like a public item,
  even when the package or crate does not export it by name: each field of a TypeScript interface
  and each public field and variant of a Rust type.
- **Self-contained**: a doc comment or README is complete inside its own package or crate. Link
  only to the published reference ([incajs.tom96da.com](https://incajs.tom96da.com/)) or to the
  official documentation of a dependency. Don't point at `handbook/`, backlog ids, planning terms
  such as phases, or files of another package.

## CLI output

- **Streams**: information goes to stdout, a warning and a failure to stderr, and `config` and
  `info`, which print data on stdout, write their information to stderr. The unpack line of the host
  binary and the host's own output go to stderr in every command.

## Differences from standards

The documentation under `docs/` covers where the host differs from the Web standards and from Vue.

- **Differences**: describe each difference and each gap that remains.
- **Standard behaviour**: don't describe a feature that follows a Web or Vue standard. Link to the
  standard's own reference when a page needs it.
- **Unspecified behaviour**: when no standard defines a behaviour, measure it in browsers and
  describe where the host behaves differently.

## Long strings

Wrap a long string with the means its language offers. Don't continue a string with `\` at the end
of a line.

| | Rust | TypeScript |
| --- | --- | --- |
| Text | `concat!` | join the parts with `+` |
| Code held in a string | raw string, `r#"..."#` | template literal |

## Dependencies

Code reaches another layer through what that layer injects.

- **Core features** (the element handles and the host calls) import no framework and not the 
  injected core.
- **Host calls**: only the host-call wrapper calls the natives. The rest of core uses the functions
  of that wrapper.
- **Frontend renderers** reach the core features only through the injected core. Only the
  composition root imports it as a value. The other files import its types and take it as a
  parameter.
- **Bundler adapters** reach the CLI only through the injected `adapterCore`, its types included.
  Only the Vite adapter imports Vite, and only the default bundler imports the adapter. The 
  adapter builds with Vite's library mode and runs the dev flow through its middleware mode and
  Runtime API. It starts no listening server.

## Names in packages

- **Packages**: kebab-case.
- **Files**: camelCase.
- **Directories**: kebab-case.
- **Export subpaths**: kebab-case, and so are the built files they name.

`oxlint` checks the file and directory names under `packages/*/src`.

## Names in crates

- **Crates**: kebab-case.
- **Modules**: snake_case.
