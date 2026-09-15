<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# @incajs/cli

The `inca` command. Internally it wires a bundler adapter's build watch to
a dev-protocol client talking to `inca-host` — both live inside this
package (`src/adapter/`, `src/dev-client/`) since neither is ever imported
on its own.

`inca dev` watches an app's entry point, starts `inca-host` once the
first bundle lands, and reloads it on every rebuild. `inca build` runs
the same pipeline once, with the watcher removed and production settings
on, and never starts a host. `inca package` builds the same way, then
pairs the bundle with a prebuilt `inca-host` into a distributable
application. `adapter/vite` is the bundler wired in by default for all
three — swapping it for another `Bundler` (the contract `adapter/types.mts`
defines) is a dependency change in `defaultBundler.mts`, not an edit
anywhere else.

## App entry

No entry point is required. Drop a `src/App.vue` and that's a whole app —
`inca` wraps it in `createIncaApp(App).mount()` (from `incajs/vue`)
itself. Commit a `src/main.mts` instead for full control over
bootstrapping; it wins outright when both exist.

## `inca package`

Emits a platform-native application into `dist/`: a `.app` on macOS, a
plain directory on Linux — other platforms aren't supported yet. Either
way, the layout puts the host binary and `bundle.js` beside each other, so
the app launches with no arguments and no terminal.

It needs a release build of the host to bundle — see `resolveHostBin`
below for how one is found.

App metadata comes from the app's own `package.json`, with an optional
`"inca"` key overriding what's derived from it:

```jsonc
{
  "name": "click_counter",
  "version": "1.0.0",
  "inca": {
    "productName": "Click Counter", // defaults to "name", scope stripped
    "identifier": "com.example.click-counter", // defaults to a generated org.inca.<slug>
    "icon": "assets/icon.icns" // resolved relative to the app's own directory
  }
}
```

An `identifier` should be world-unique, so `inca package` prints a note
when it falls back to the generated one rather than using it silently.

## `dev-client`

The Node end of the dev protocol: resolves and spawns `inca-host --dev
<bundle>`, and speaks the newline-delimited JSON-RPC 2.0 channel it
answers on.

`HostClient` correlates each request it sends with the response that
answers it and relays the host's stderr. Two notification methods carry
meaning of their own: `ready`, whose `params.protocol` it checks against
the revision it was built for, reporting and terminating the child on any
mismatch; and `appError`, an app fault the host caught and kept rendering
past, handed to its own callback rather than a generic one. Any other
method name is routed, `params` untouched and unparsed, to whichever
integration the caller registered for it.

A line the host writes that doesn't parse as a JSON-RPC message is treated
as stray output from a dependency, not a protocol violation: it's logged
and the channel keeps reading.

`resolveHostBin` tries, in order:

1. `INCA_HOST_BIN`, if set — names the binary to spawn outright. This is
   the escape hatch for a platform with no published binary yet, or a
   custom-built one.
2. The `optionalDependency` matching this OS/arch
   (`@incajs/host-darwin-arm64`, `-darwin-x64`, `-linux-arm64`,
   `-linux-x64` — Windows isn't supported yet), which almost every install
   resolves through without either side ever needing Cargo or Rust.

@throws if neither resolves.

## `adapter/vite`

Bundles an app's entry point into one self-contained bundle that
`inca-host` can evaluate: no unresolved imports, no dependency on
QuickJS having Node.js globals.

`watch(options)` builds `options.entry` into a bundle under
`options.outDir` — the returned `Watcher`'s `bundlePath` names the exact
file — rebuilding it on every change and reporting each result through
`options.onBuild`/`options.onError`. `build(options)` runs the same
pipeline once, minified and with `outDir` cleared first, resolving with
the bundle's path or rejecting on failure rather than reporting it
through a callback.

The template compiler is retargeted at `@vue/runtime-core` — the only Vue
runtime package `incajs/vue` itself depends on — instead of the default
`vue` import. `@vitejs/plugin-vue` itself still imports `vue` directly for
its own internals, which is why it's a peer dependency of this package:
that build-time need for `vue` never reaches an app's dependency tree or
the bundle it produces.
