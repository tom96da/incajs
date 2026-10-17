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
- **`//` comments** say Why not. One line states the constraint that rules
  out the obvious alternative.

## Enforcement

A convention that a tool can check is enforced by one of two tools.

- **`oxlint`** checks the code against rules, configured in
  [`oxlint.config.ts`](../oxlint.config.ts).
- **`oxfmt`** formats the code, configured in
  [`oxfmt.config.ts`](../oxfmt.config.ts).

A convention without a rule is a review criterion.

## Prose

Comments, documentation and commit messages state the facts that hold.

- **Plain wording**: write the fact directly. Don't add a framing clause
  that softens or decorates it.
- **Present behaviour**: describe what the code does. Don't write
  counterfactuals or the history of earlier versions.
- **Positive statements**: write what holds. Don't write "not X but Y",
  "instead of X" or "rather than X".

## Public API docs

rustdoc and TSDoc on a public item are reference material for the framework
user.

- **Reference**: describe what the item does, its contract and its errors,
  and when to pass an option. Don't explain how it is implemented.
- **Parameters and results**: document each parameter and each return value.
  Don't restate what the name already says.
- **Self-contained**: a doc comment or README is complete inside its own
  package or crate. Link only to the published reference
  ([incajs.tom96da.com](https://incajs.tom96da.com/)) or to the official
  documentation of a dependency. Don't point at `handbook/`, backlog ids,
  planning terms such as phases, or files of another package.

## Internal items

The parameters, return values, fields and methods of an item that users never
see have a description where the name leaves something unsaid. The
description states that. Don't rephrase the name.

## Differences from standards

The documentation under `docs/` covers where the host differs from the Web
standards and from Vue.

- **Differences**: describe each difference and each gap that remains.
- **Standard behaviour**: don't describe a feature that follows a Web or Vue
  standard. Link to the standard's own reference when a page needs it.
- **Unspecified behaviour**: when no standard defines a behaviour, measure it
  in browsers and describe where the host behaves differently.

## Long strings

Wrap a long string with the means its language offers. Don't continue a
string with `\` at the end of a line.

| | Rust | TypeScript |
| --- | --- | --- |
| Text | `concat!` | join the parts with `+` |
| Code held in a string | raw string, `r#"..."#` | template literal |

## Dependencies

Code reaches another layer through what that layer injects.

- **Core features** (the element handles, the shadow tree and the host calls)
  import no framework and not the injected core.
- **Frontend renderers** reach the core features only through the injected
  core. Only the composition root imports it as a value. The other files
  import its types and take it as a parameter.
- **Bundler adapters** reach the CLI only through the injected `adapterCore`,
  its types included.
  Only the Vite adapter imports Vite, and only the default bundler imports the
  adapter. The adapter builds with Vite's library mode and runs the dev flow
  through its middleware mode and Runtime API. It starts no listening server.

## Names in packages

- **Packages**: kebab-case.
- **Files**: camelCase.
- **Directories**: kebab-case.
- **Export subpaths**: kebab-case, and so are the built files they name.

`oxlint` checks the file and directory names under `packages/*/src`.

## Names in crates

- **Crates**: kebab-case.
- **Modules**: snake_case.
