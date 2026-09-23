<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Failures

When this framework refuses something, when it carries on with a default,
and where each is reported. Applies to both languages, the same way
[TESTING.md](./TESTING.md) covers both.

## Refuse, or fall back

The axis is what the app said, not how bad the outcome is.

| The app | What happens |
| --- | --- |
| said nothing | the documented default, in silence |
| said something unusable | a failure, named |

A `width` nobody set falls through to the app's root element and then to
800 × 600, because that chain is the documented behaviour of leaving it
out. A `width` of `-100` is a typo: it is refused, and the message says
which value and which file.

The same line divides the two halves of `productName`. Absent, `inca dev`
reports it and runs the app unnamed. Holding a path separator, it fails —
that name becomes a directory.

A feature the engine can't run yet is refused, not ignored: a `<style>`
block fails the build from the one list in
`packages/cli/src/adapter/vite/unsupported.mts`, rather than reaching the
screen as nothing. Silence there would leave an app looking wrong with no
way to tell why.

## Where a failure is reported

| Raised by | Carried as |
| --- | --- |
| `@incajs/cli` | an `IncaError` with an `ERR_INCA_*` code, printed by `printFault` |
| `inca-host`, answering a protocol message | a JSON-RPC error — see [PROTOCOL.md](./PROTOCOL.md#failure-handling) |
| `inca-host`, otherwise | its stderr |
| an app's own event listener | an `appError` notification |

A new `ERR_INCA_*` code is documented in `docs/reference/errors.md` in the
change that raises it. The code is what someone searches for.

## What holds whatever happens

- `inca-host` doesn't exit over a message it couldn't use, and doesn't
  panic on a failure an app's author can cause.
- A reload evaluates the new bundle before swapping the window over, so a
  broken edit leaves the last working UI on screen.
- An exception from an event listener is reported and the window keeps
  rendering.

## Known gaps

`inca dev` exits when it fails before its watcher starts — an unreadable
`inca.config.ts`, or no entry to build. A failure after that point is
reported and watched through, so the same edit-and-retry loop doesn't
apply to both. Phase 3.4's `watchConfig` unit is where that closes.

A packaged app's stderr reaches nobody, so neither its `console` output nor
the host's own reports survive outside `inca dev`. Recorded in
[BACKLOG.md](./BACKLOG.md).
