<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-jsenv

The `QuickJS` runtime bootstrap (`Engine`), plus the host objects installed
into a `QuickJS` realm so ordinary JavaScript can run in it. It also builds
source text into an async function whose stack frames name its file. An
inline source map in that text points the frames at the original source.

`QuickJS` supplies the language: `Array`, `JSON`, `Promise`, `Math` and the
rest of the ECMAScript intrinsics. It supplies nothing else: no `console`,
no timers, no network, no filesystem. Those are the host's to provide, and
this crate is where they live.

Today that is `console`. It writes wherever the caller points it, because
the process embedding it may already be using stdout for something else.

```rust
use std::rc::Rc;
use inca_jsenv::console;

// `ctx` is an `rquickjs::Ctx`.
let to_stderr: console::Output = Rc::new(|_, line: &str| eprintln!("{line}"));
console::install(&ctx, &to_stderr, false)?;
ctx.eval::<(), _>("console.log('ready', { count: 1 })")?;
// stderr: ready { count: 1 }
```

## Host values

`vault(&ctx)` returns a null-prototype object of the realm that host code
keeps JS values in between calls, under plain keys. An `Engine` creates it
before any script runs. A script that calls the global holding it gets
`undefined`, and cannot replace or delete that global.

## Accessors from functions

`FunctionAccessor` defines an accessor property whose getter and setter are
existing functions, so one named getter can serve many objects.

## Module loading

`Engine::builder().module_root(dir)` adds a directory `import`s resolve
against. A bare specifier is searched for under each root, in the order
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

## Compiling a function

`compile_async_function` takes parameter names, a body, and the file name
to report. It returns the async function as a `Function`.

```rust
use inca_jsenv::compile_async_function;

// `ctx` is an `rquickjs::Ctx`; `body` is the source text.
let f = compile_async_function(&ctx, &["a", "b"], body, "app/widget.js")?;
```

The stack frames that `EngineError` and `console` report name `app/widget.js`,
and a frame on line `N` of `body` reports line `N + 1`. When `body` ends with
an inline `//# sourceMappingURL=data:` source map, those frames name the
original file, line and column. Each compile keeps its own map for the life of
the context, so a function from an earlier compile of the same file name keeps
its source locations after a recompile. The `stack` property a script reads
keeps the generated positions. It names the file `app/widget.js` for the first
compile and `app/widget.js?t=N` for a later one, where `N` is unique to the
compile. A syntax error is left pending on `ctx`, where
`EngineError::capture` reads it.

## Scope

The renderer, windows and processes are outside it. The runtime and what it
installs are replaceable, either by a third-party implementation or by an
application's own, without any of that reaching the code that draws a UI.
