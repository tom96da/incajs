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
use inca_jsenv::console;

// `ctx` is an `rquickjs::Ctx`.
console::install(&ctx, &console::to_stderr())?;
ctx.eval::<(), _>("console.log('ready', { count: 1 })")?;
// stderr: log: ready { count: 1 }
```

## Module loading

`Engine::builder().module_root(dir)` adds a directory `import`s resolve
against — a bare specifier is searched for under each root, in the order
added, and a `./`/`../`-relative one resolves against the importing
module's own path. `eval_module` declares and runs a module by name,
driving its top-level evaluation (including a top-level `await`) to
completion:

```rust
use inca_jsenv::Engine;

let engine = Engine::builder().module_root("src").build()?;
engine.eval_module("src/main.js", "import './helper.js';")?;
```

A module whose top-level `await` settles later runs in two steps.
`start_module` evaluates it up to the first `await` and returns a
`PendingModule`. `poll_module` runs promise jobs and returns `None` while the
module is pending, then `Some(result)` once it has settled. `eval_module` is
`start_module` plus one `poll_module`, and a module still pending after that
poll is an error.

```rust
let source = "await new Promise((resolve) => { globalThis.release = resolve; });";
let module = engine.start_module("src/main.js", source)?;
let result = loop {
    if let Some(result) = engine.poll_module(&module) {
        break result;
    }
    // Run code that resolves what the module awaits.
    engine.eval::<()>("release();")?;
};
result?;
```

## Scope

This crate depends on `rquickjs` and nothing else — not on the renderer, not
on any window or process. That is deliberate: the runtime and what it
installs are replaceable, either by a third-party implementation or by an
application's own, without any of that reaching the code that draws a UI.
