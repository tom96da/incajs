---
description: Where the globals the host provides differ from the Web standards, and which standard ones are missing.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Built-ins

The host defines these globals before your app runs. Each section lists where
a global differs from the Web standards. [Console](./console) has its
own page.

## `TextEncoder` and `TextDecoder`

Both follow the
[Encoding Standard](https://encoding.spec.whatwg.org/).

- `TextEncoderStream` and `TextDecoderStream` are missing.
- The host refuses two cases of a detached buffer:
  - `encodeInto()` with a view over a detached resizable buffer throws a
    `TypeError`.
  - `decode()` returns an empty string when a `stream` getter of the options
    detaches the buffer of the view.

## `URL` and `URLSearchParams` <Badge type="warning" text="unreleased"/>

Both follow the
[URL Standard](https://url.spec.whatwg.org/).

- `URL.createObjectURL()` and `URL.revokeObjectURL()` are missing, because the
  host has no `Blob`.
- `new URLSearchParams(record)` reads a `Proxy` record in this order: every
  `getOwnPropertyDescriptor` trap, then every `get` trap.
- A URL of 4 GiB or more aborts the host.

These results of parsing and setting differ from the ones of browsers:

- The characters `^` and `|` stay raw in a path.
- Setting `pathname` to `//x` on `foo:/p` gives `foo://x`.
- A space in a host and the label `xn--a` are refused.
- Setting `protocol` to `http` on `mailto:x?q#f` changes nothing.
- The `origin` of `filesystem:http://a/t/x` is `"null"`.
- On `data:a ?x`, `searchParams.delete("x")` and `search = ""` give `data:a`.
  On `data:a #x`, `hash = ""` gives `data:a`.

These results for `file:` URLs differ from the ones of browsers:

- `file:///C|/x` keeps the `|`.
- `file://C:/x` parses.
- The host `localhost` is dropped, also by the `hostname` setter.
- `file:///c:/../x` and `/x` against `file:///C:/a/b` keep the drive letter.

## `performance`

- `now()` is the only member. It counts milliseconds from the start of the
  app, and a reload of the development window starts it again.

## `setTimeout()` and `setInterval()`

These follow the
[HTML Standard](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers).

- Their types come with `incajs`.
- A handler must be a function. A string or any other value throws a
  `TypeError`.
- An interval keeps its schedule. After a late run, the next run is the next
  point of the original schedule, and the runs it missed are skipped.

## `requestAnimationFrame()`

This follows the
[HTML Standard](https://html.spec.whatwg.org/multipage/imagebitmap-and-animations.html#animation-frames).

- Its type comes with `incajs`.
- A callback must be a function. A string or any other value throws a
  `TypeError`.
- On macOS, callbacks wait while another window fully covers the app's window,
  and run when it is uncovered.

## Limits

- One run of the app can register 2^31 - 1 timers, frame callbacks and event
  listeners in total. A cleared or removed one still counts. After that, these
  throw a `RangeError`:
  - `setTimeout()`
  - `setInterval()`
  - `requestAnimationFrame()`
  - `addEventListener()`
- A reload of the development window starts the count again.
