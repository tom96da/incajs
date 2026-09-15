<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-jsenv

The `QuickJS` runtime bootstrap (`Engine`), plus the host objects installed
into a `QuickJS` realm so ordinary JavaScript can run in it.

`QuickJS` supplies the language — `Array`, `JSON`, `Promise`, `Math` and the
rest of the ECMAScript intrinsics. It supplies nothing else: no `console`,
no timers, no network, no filesystem. Those are the host's to provide, and
this crate is where they live.

Today that is `console`. It writes wherever the caller points it, because
the process embedding it may already be using stdout for something else.

```rust
use std::rc::Rc;
use inca_jsenv::console;

// `ctx` is an `rquickjs::Ctx`.
console::install(&ctx, console::to_stderr())?;
ctx.eval::<(), _>("console.log('ready', { count: 1 })")?;
// stderr: log: ready { count: 1 }
```

## Scope

This crate depends on `rquickjs` and nothing else — not on the renderer, not
on any window or process. That is deliberate: the runtime and what it
installs are replaceable, either by a third-party implementation or by an
application's own, without any of that reaching the code that draws a UI.
