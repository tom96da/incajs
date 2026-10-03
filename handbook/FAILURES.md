<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Failures

When the framework refuses something, when it carries on with a default,
and where each is reported.

## Refuse, or fall back

| The app | What happens |
| --- | --- |
| said nothing | the documented default, in silence |
| said something unusable | a failure, named |

For example, a `width` nobody set falls through to the app's root element
and then to 800 × 600.

A feature the engine cannot run yet fails the build. A `<style>` block is one
(the list is `packages/cli/src/adapter/vite/unsupported.mts`).

## Where a failure is reported

| Raised by | Carried as |
| --- | --- |
| `@incajs/cli` | an `IncaError` with an `ERR_INCA_*` code, printed by `printFault` |
| `inca-host`, answering a protocol message | a JSON-RPC error (see [PROTOCOL.md](./PROTOCOL.md#failure-handling)) |
| `inca-host`, otherwise | its stderr |
| a panic in `inca-host` under `--dev` | a `-32603` error, printed by `inca dev` as `[host error -32603]` |
| an app's own event listener, promise job or unhandled rejection | an `appError` notification |
| `inca-host` exiting before `ready` | `ERR_INCA_HOST_EXITED_EARLY`, after `inca dev` prints the cause |
| `inca-host` exiting after `ready` with a non-zero code or a signal other than SIGINT/SIGTERM | `ERR_INCA_HOST_CRASHED`, after `inca dev` prints `host exited` |

A new `ERR_INCA_*` code is documented in `docs/reference/errors.md` in the
change that raises it.

## Guarantees

`inca-host` does not panic on a failure an app's author can cause.
