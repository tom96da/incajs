<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# hello_world

A Vue port of [`crates/inca-gpui/examples/hello_world.rs`](../../crates/inca-gpui/examples/hello_world.rs):
a static tree — a bordered box, a text label, and a row of six colored
squares — built as a real `.vue` SFC (`src/App.vue`) instead of Rust
`VirtualTree` calls.

```sh
cargo build -p inca-host
INCA_HOST_BIN="$(pwd)/target/debug/inca-host" pnpm --filter hello_world dev
```

`inca dev` (from [`@incajs/cli`](../../packages/cli/README.md)) builds
`src/App.vue` and every rebuild after, starts
[`inca-host`](../../crates/inca-host/README.md) once the first build
lands, and reloads it on every following one.

`pnpm --filter hello_world build` runs the same build once, minified and
without starting a host, writing `dist/bundle.js`. Launch the built bundle
directly, without the dev protocol, with:

```sh
cargo run -p inca-host -- examples/hello_world/dist/bundle.js
```

`pnpm --filter hello_world package` (see
[`inca package`](../../packages/cli/README.md#inca-package)) pairs that
same build with a prebuilt `inca-host` into a distributable application
at `examples/hello_world/dist/hello_world.app` (macOS) or
`examples/hello_world/dist/hello_world/` (Linux) — double-click it, or
launch it directly:

```sh
cargo build -p inca-host --release
INCA_HOST_BIN="$(pwd)/target/release/inca-host" pnpm --filter hello_world package
./examples/hello_world/dist/hello_world/hello_world  # Linux
open examples/hello_world/dist/hello_world.app        # macOS
```
