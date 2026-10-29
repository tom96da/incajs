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
| shipped a source map the host cannot read | the frames keep the generated positions, in silence |

For example, a `width` nobody set falls through to the app's root element
and then to 800 × 600.

A feature the engine cannot run yet fails the build. A `<style>` block is one
(the list is `packages/cli/src/vite/unsupported.mts`).

## Where a failure is reported

| Raised by | Carried as |
| --- | --- |
| `@incajs/cli` | an `IncaError` with an `ERR_INCA_*` code, printed by `printFault` |
| the host's `--print-config` | `ERR_INCA_CONFIG_INVALID`, and the command exits 1 |
| `inca-host`, answering a protocol message | a JSON-RPC error (see [PROTOCOL.md](./PROTOCOL.md#failure-handling)) |
| `inca-host`, otherwise | its stderr |
| a script that runs past the memory or the stack limit of the engine | an `appError` notification under `--dev`, stderr in a packaged app; a stop during the first load fails the start with exit 1 |
| a panic in `inca-host` under `--dev` | a `-32603` error, printed by `inca dev` as `[host error -32603]` |
| a panic in `inca-host` of a packaged app | `panic.log` in the log directory of the app, and stderr |
| an error the running app raised and the host caught | an `appError` notification, see [PROTOCOL.md](./PROTOCOL.md#messages) |
| a config or entry failure before the first build | `build failed`, then a retry on the next edit |
| `inca-host` exiting before `ready`, which includes a first bundle that throws or rejects | `ERR_INCA_HOST_EXITED_EARLY`, after `inca dev` prints the cause |
| `inca-host` exiting after `ready` with a non-zero code or a signal other than SIGINT/SIGTERM | `ERR_INCA_HOST_CRASHED`, after `inca dev` prints `host exited` |

## Guarantees

`inca-host` does not panic on a failure an app's author can cause.
