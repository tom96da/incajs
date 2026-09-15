<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Dev protocol (host ↔ dev-client)

The message surface between `gpjs-ui-host` and the Node process that spawns
it during development. `@incajs/cli`'s `dev-client` owns that end — it
resolves and launches the host binary and speaks everything below, driven by
the CLI's own commands. The counterpart to [FFI.md](./FFI.md), which covers
the other boundary — JS calling into Rust inside the host's own process.

See [AGENTS.md](../AGENTS.md#status) for how much of this is built. Update
this file whenever a message lands or changes, same as [FFI.md](./FFI.md).

## Transport

Dev mode is opt-in:

```sh
gpjs-ui-host --dev <path-to-bundle.js>
```

Without `--dev` the host reads no stdin and writes no protocol messages —
the one-shot behaviour its README describes.

| Stream | Direction | Carries |
| --- | --- | --- |
| host stdin | client → host | protocol messages |
| host stdout | host → client | protocol messages, and nothing else |
| host stderr | host → client | human-readable logs, and the running app's own `console` output |

One JSON object per line, UTF-8, `\n`-terminated. **stdout is the protocol
channel**: a stray `println!` corrupts it, so every diagnostic goes to
stderr — the host's own logs and, verbatim, whatever the app writes through
`console`.

A client drains stdout for as long as the child lives. The host answers on
the thread that runs the app, so a client that stops reading eventually
stops the host.

The host writes nothing else to stdout, but it cannot vouch for a dependency
that does. A reader therefore treats a line it can't parse as JSON, or one
that parses without a `jsonrpc` member, as stray output — log it to stderr
and carry on, never fail on it. Testing for a leading `{` before parsing
skips almost all of that at no cost.

## Envelope

[JSON-RPC 2.0](https://www.jsonrpc.org/specification), one object per line.

```json
{"jsonrpc": "2.0", "id": 1, "method": "reload"}
{"jsonrpc": "2.0", "id": 1, "result": null}
{"jsonrpc": "2.0", "method": "ready", "params": {"protocol": 0}}
```

JSON-RPC leaves framing to the transport. This one is newline-delimited, so
a line is a message.

- A request carries an `id`; its response echoes that `id` unchanged, so
  answers stay matched to their requests when several are in flight.
- A **notification** is a call with no `id` at all, and takes no response.
  An explicit `"id": null` is a request, not a notification.
- An unrecognized `method` is answered `-32601` when it arrives as a request
  and dropped when it arrives as a notification, so either side can add
  methods first.
- `params` this host has no use for are ignored rather than rejected.

## Messages

### To the host

| `method` | Response | Meaning |
| --- | --- | --- |
| `reload` | `null` on success | Re-read the bundle at the path given on argv and evaluate it afresh. |
| `shutdown` | `null` | Exit 0. |

### From the host

| `method` | Kind | Meaning |
| --- | --- | --- |
| `ready` | notification | The window is open and the first bundle has been evaluated. `params.protocol` is this host's method-set revision. |
| `appError` | notification | The running app raised something the host caught and recovered from. `params` carries the thrown value. |

`params.protocol` versions the method set, not JSON-RPC itself. Phase 3.3
publishes the client and the host binary separately, so the pair can be
mismatched: a client compares this against the revision it was built for and,
on any difference, reports it on its own stderr and terminates the child.
Nothing negotiates — a difference means the installed pair is wrong. A
client depends on an exact host build rather than a range, so a mismatch is
a broken installation and this check is what says so out loud.

`appError` is how a fault inside the running app reaches a human. An
exception thrown by an event listener answers no request — nothing asked for
it — and must not take the window down, so the host catches it, keeps
rendering, and reports it here. Without a message of its own such a fault is
invisible: it belongs to no `id`, and a client can't pick one out of a log
stream.

`params` is `{"message": string, "stack": string | null}`. `message` is the
thrown value as text; `stack` is its call stack, and is `null` whenever there
is none — JS can throw any value, and a thrown string or object carries no
stack at all. Keeping them apart lets a client fold a long stack away instead
of splitting one blob back up.

The app's own `console` calls are not this message. Those are a log stream
and go to stderr. `appError` is a fault.

`shutdown` needs no reply beyond its response: the child's exit is the real
acknowledgement. The client kills the child if it hasn't exited by then, so
cleanup gets a chance to run without a wedged app being able to block
Ctrl-C.

## Extending it

Two surfaces grow here, and neither needs the other to change.

**New methods.** Add one and document it above. An unrecognized method is
answered `-32601` rather than closing the connection, so the two sides can be
upgraded separately.

**New bundler integrations.** Bundler traffic rides this channel in both
directions as notifications whose `method` names the producer and whose
`params` carry the payload untouched: Vite's `ModuleRunnerTransport` frames
arrive as `{"method": "vite", "params": {...}}` in Phase 3.4. A `fetchModule`
starts in the host and is answered by the client, and an HMR update travels
the other way — both as `vite` notifications, because Vite pairs a call with
its answer by an id it keeps inside `params`. Nesting therefore keeps their
shape intact and asks nothing of this layer's own `id`.
`dev-client` depends on no bundler and reaches a payload's owner only
through the handle the CLI's own commands pass it, so another integration is
a new `method` name, not a change here.

**New app-visible events.** These don't travel this protocol at all. The
host dispatches any `(node id, event name)` pair to whatever JS registered
for it through `__gpjsui_native__.addEventListener`, and `rootNodeId()` gives
an app a target not tied to any element, so an app lifecycle hook — cleanup
before `shutdown`, a warning before a reload discards state — would be a name
the host agrees to dispatch, not a new binding and not a new message. **No
lifecycle name is defined and nothing dispatches one**; a click on an element
is the only event that reaches JS today, and [FFI.md](./FFI.md) is where a
lifecycle surface gets settled.

## Failure handling

The host never exits because of a message it couldn't use.

| Situation | Code | Response |
| --- | --- | --- |
| A line that isn't valid JSON | `-32700` | `id` is `null` — there was none to read |
| Valid JSON that is no request object: not an object, no `jsonrpc: "2.0"`, no readable `method`, or an `id` that is not a string, a number, or null | `-32600` | `id` is `null` |
| A `method` this host doesn't implement | `-32601` | echoes the request's `id`; nothing at all if it was a notification |
| A bundle that throws while being evaluated | `-32000` | echoes the `id`; the window keeps the tree it already has |
| A *first* bundle that throws, before any window exists | `-32000` | reported with `id` `null`, then exit 1 |
| An exception thrown by an app's event listener | — | an `appError` notification; the window keeps rendering |

`-32000` is inside the range JSON-RPC reserves for application-defined
errors; the rest are the spec's own. It splits a thrown JS value the same way
`appError` does: `message` is the value as text, and `data` is
`{"stack": string | null}`.

Every response a client waits for needs a deadline. The host answers on the
same thread that runs the app's JS, so an app stuck in a loop stops
answering everything, `shutdown` included — ending that is the client's job,
not something a further message can reach.

A reload evaluates the new bundle into a fresh `Engine` and `Host`, and swaps
the window over only once that succeeds. A broken edit therefore leaves the
last working UI on screen instead of blanking the window.
