<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Dev protocol (host ↔ dev-client)

The messages `inca-host` and the Node process that spawns it exchange during
development. `@incajs/cli`'s `dev-client` is the Node end: it launches the
host binary and speaks every message below. [FFI.md](./FFI.md) covers the
other boundary, JS calling into Rust inside the host process.

## Transport

Dev mode is opt-in:

```sh
inca-host --dev <path-to-bundle.js>
```

Without `--dev` the host reads no stdin and writes no protocol messages, as
described in `crates/inca-host/README.md`.

| Stream | Direction | Carries |
| --- | --- | --- |
| host stdin | client → host | protocol messages |
| host stdout | host → client | protocol messages only |
| host stderr | host → client | human-readable logs, and the running app's own `console` output |

One JSON object per line, UTF-8, `\n`-terminated. Diagnostics go to stderr.

A client drains stdout while the child lives. The host writes through an
unbounded channel on its own thread, so a client that stops reading does not
stall the app, and the host buffers without a limit.

A client logs a stdout line that is not a JSON-RPC object to stderr and
carries on.

## Envelope

[JSON-RPC 2.0](https://www.jsonrpc.org/specification), one object per line.

```json
{"jsonrpc": "2.0", "id": 1, "method": "reload"}
{"jsonrpc": "2.0", "id": 1, "result": null}
{"jsonrpc": "2.0", "method": "ready", "params": {"protocol": 0}}
```

- A request carries an `id`. Its response echoes the `id`, so answers stay
  matched when several requests are in flight.
- A notification has no `id` and takes no response. `"id": null` makes it a
  request.
- An unrecognized `method` in a request is answered `-32601` with the
  message `unknown method: <method>`. In a notification it is relayed into
  the running app (see "Extending it").
- Unused `params` are ignored.

## Messages

### To the host

| `method` | Response | Meaning |
| --- | --- | --- |
| `reload` | `null` on success | Re-read the bundle at the path given on argv and evaluate it afresh. The config read at startup stands. |
| `shutdown` | `null` | Exit 0. The client kills the child if it has not exited after the response. |

### From the host

| `method` | Kind | Meaning |
| --- | --- | --- |
| `ready` | notification | The window is open and the first bundle has been evaluated. `params.protocol` is the host's method-set revision. A client compares it with the revision it was built for. On a mismatch it reports on its own stderr and terminates the child. |
| `appError` | notification | The running app raised something the host caught and recovered from: an exception thrown by an event listener, a failing promise job, or a promise rejected with no handler. `params` is `{"message": string, "stack": string \| null}`. `message` is the thrown value as text, and `stack` is `null` when the value has none. The app's own `console` output goes to stderr. |

## Extending it

**New methods.** Add one and document it under Messages. An unrecognized
method follows the Envelope rules, so the two sides can be upgraded
separately.

**Bundler traffic.** A bundler integration sends notifications whose `method`
names the producer and whose `params` carry the payload. The host relays a
notification with an unrecognized `method` into the running app as
`globalThis.__inca_dev__.receive?.(method, paramsJson)`, where `paramsJson`
is `params` as a JSON string. A relay that throws, or finds no
`__inca_dev__.receive`, is reported as an event listener's throw is.

The app sends a notification back with `__inca_dev__.send(method,
paramsJson)`. It parses `paramsJson` as JSON and writes it out as a
notification named `method`. Invalid JSON raises a `TypeError`.
`__inca_dev__` exists only in dev builds.

Vite's `ModuleRunnerTransport` frames travel as `{"method": "vite",
"params": {...}}` notifications in both directions. A `fetchModule` goes from
the host to the client, and an HMR update goes from the client to the host.
`dev-client` depends on no bundler. The CLI's own commands pass it the handle
that owns a payload.

## Failure handling

The host never exits because of a message it couldn't use.

| Situation | Code | Response |
| --- | --- | --- |
| A line that isn't valid JSON | `-32700` | `id` is `null`, as there was none to read |
| Valid JSON that is no request object: not an object, no `jsonrpc: "2.0"`, no readable `method`, or an `id` that is not a string, a number, or null | `-32600` | `id` is `null` |
| A `method` this host doesn't implement | `-32601` | echoes the request's `id` |
| A panic in the host | `-32603` | `id` is `null`. The panic message also goes to stderr |
| A bundle that throws while being evaluated | `-32000` | echoes the `id`. The window keeps the tree it already has |
| A *first* bundle that throws, before any window exists | `-32000` | reported with `id` `null`, then exit 1 |
| An exception the running app raised and the host caught | — | an `appError` notification, and the window keeps rendering. See Messages. |

`-32000` carries a thrown JS value: `message` is the value as text, and
`data` is `{"stack": string | null}`.

The host answers on the thread that runs the app's JS, so an app stuck in a
loop answers nothing, `shutdown` included. A client times out and kills the
child.

A reload evaluates the new bundle into a fresh `Engine` and `Host`, and swaps
the window over only if that succeeds.

## App config (`inca.json`)

A build writes `inca.json` beside the entry:

```json
{
  "name": "Demo",
  "identifier": "org.inca.demo",
  "window": { "width": 1024, "height": 768, "title": "Demo" }
}
```

Every member is optional, and a build that has none of them writes no file.
`name` is what the platform calls the running app, `identifier` its
reverse-DNS id, and `window` the size and title it opens at.

The host reads it once, at startup, before opening the window. It reaches
the file the same way whether a client spawned it or a person double-clicked
a packaged app. A missing file reads as the defaults in silence. A file that
cannot be read or parsed is named on the host's stderr and reads as the
defaults too.
