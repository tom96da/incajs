# hello_world

A Vue port of [`crates/gpjs-ui/examples/hello_world.rs`](../../crates/gpjs-ui/examples/hello_world.rs):
a static tree — a bordered box, a text label, and a row of six colored
squares — built as a real `.vue` SFC (`src/App.vue`) instead of Rust
`VirtualTree` calls.

```sh
cargo build -p gpjs-ui-host
pnpm --filter hello_world dev
```

`gpjsui dev` (from [`@incajs/cli`](../../packages/cli/README.md)) builds
`src/App.vue` and every rebuild after, starts
[`gpjs-ui-host`](../../crates/gpjs-ui-host/README.md) once the first build
lands, and reloads it on every following one.

`pnpm --filter hello_world build` runs the same build once, minified and
without starting a host, writing `dist/bundle.js`. Launch the built bundle
directly, without the dev protocol, with:

```sh
cargo run -p gpjs-ui-host -- examples/hello_world/dist/bundle.js
```

`pnpm --filter hello_world package` (see
[`gpjsui package`](../../packages/cli/README.md#gpjsui-package)) pairs that
same build with a prebuilt `gpjs-ui-host` into a distributable application
at `examples/hello_world/dist/hello_world.app` (macOS) or
`examples/hello_world/dist/hello_world/` (Linux) — double-click it, or
launch it directly:

```sh
cargo build -p gpjs-ui-host --release
pnpm --filter hello_world package
./examples/hello_world/dist/hello_world/hello_world  # Linux
open examples/hello_world/dist/hello_world.app        # macOS
```
