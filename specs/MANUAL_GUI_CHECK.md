<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Manually verifying a GPUI window opens

Some PLAN.md tasks include a manual checklist item like "run the example
and look at the window" — a human visually confirming a real window opens
and renders, which an agent working inside the devcontainer can't do on its
own (no display attached — see [why](#why-an-agent-cant-just-do-this)
below). This doc covers how to do that check
yourself, for the existing examples
(`crates/inca-gpui/examples/gpui/hello_world.rs`,
`crates/inca-gpui/examples/hello_world.rs`,
`crates/inca-gpui/examples/click_counter.rs`, and the two Vue ports,
`examples/hello_world` and `examples/click_counter`, run through
`inca-host`) and any future one.

There are two ways to see the window, depending on which platform's
rendering backend you want to exercise. Option A is simpler and is enough
for most checks; use Option B when you specifically want to exercise the
Linux backend (e.g. because that's what CI/the devcontainer itself runs on).

## Option A — run natively on macOS (recommended)

The devcontainer just bind-mounts this repo into a container; the files
also exist on the host at whatever path you opened in VS Code. `gpui`'s
Cargo dependency graph already branches per OS (macOS uses `gpui_apple` /
Metal, Linux uses `gpui_linux` / Vulkan), so running the same `cargo`
command directly on macOS, outside the container, gets you the native
backend with no extra setup:

```sh
cargo run -p inca-gpui --example gpui_hello_world
```

A window with a gray background, the text "Hello, World!", and a row of six
colored boxes should appear.

### The Vue ports (`examples/hello_world`, `examples/click_counter`)

These aren't Cargo examples — build `inca-host` once, then let `inca
dev` build the `.vue` app and start it:

```sh
cargo build -p inca-host
pnpm --filter hello_world dev
```

Same look as `hello_world`/`gpui_hello_world` above. Swap in
`click_counter` for the clickable, counting box (same look as
`click_counter.rs`) — clicking it should count up, confirming
`EventDispatcher` correctly drains `@vue/runtime-core`'s
microtask-scheduled reactivity update (see [PLAN.md](./PLAN.md)'s Unit iv notes).

### A packaged app (`inca package`)

Confirms the same thing about a distributable `.app`, launched the way a
real user would rather than through `cargo run`/`inca dev`:

```sh
cargo build -p inca-host --release
pnpm --filter click_counter package
open examples/click_counter/dist/click_counter.app
```

Double-click it from Finder instead if you want to also confirm it starts
with no terminal attached at all. Same look and click behavior as the dev
run above — this is the check [PLAN.md](./PLAN.md)'s Phase 3.3 Unit ii still
needs on macOS specifically (the exe-relative bundle search itself is
already confirmed on Linux, inside the devcontainer, with no display to
carry it further).

### No text, but the background/boxes render fine

`gpui_platform`'s `font-kit` feature isn't enabled for this platform (see
`crates/inca-gpui/Cargo.toml`) — without it, `gpui_macos` silently skips all
text rendering while drawing everything else normally, with no error.

### Metal toolchain errors

If this fails with something like:

```
error: cannot execute tool 'metal' due to missing Metal Toolchain; use: xcodebuild -downloadComponent MetalToolchain
```

or

```
xcrun: error: unable to find utility "metal", not a developer tool or in PATH
```

`gpui_apple`'s build script needs the `metal` shader compiler, which ships
with the full Xcode.app, not the standalone Command Line Tools. Fix:

1. Install Xcode from the App Store if you haven't.
2. Point the active developer directory at it:
   ```sh
   sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
   sudo xcodebuild -license accept
   ```
3. Download the Metal toolchain component:
   ```sh
   xcodebuild -downloadComponent MetalToolchain
   ```
   This downloads ~688 MB. On macOS 15 (Sequoia) and later, you may see a
   log line like:
   ```
   Metal Toolchain unable to refresh cache with error: … "Operation not permitted"
   ```
   The download still completes, but the automatic installation step is
   blocked by SIP. In that case, install the toolchain manually:
   ```sh
   # 1. Find the downloaded DMG (path will contain a hash-like directory name)
   DMG=$(find /System/Library/AssetsV2/com_apple_MobileAsset_MetalToolchain \
         -name "*.dmg" 2>/dev/null | head -1)

   # 2. Mount it
   hdiutil attach "$DMG" -nobrowse

   # 3. Copy the toolchain into your user toolchains directory
   mkdir -p ~/Library/Developer/Toolchains
   cp -R /Volumes/MetalToolchainCryptex/Metal.xctoolchain \
         ~/Library/Developer/Toolchains/

   # 4. Unmount
   hdiutil detach /Volumes/MetalToolchainCryptex
   ```
4. Confirm: `xcrun -sdk macosx metal --version` should print a version line.

If `xcrun`/`xcodebuild` itself errors with a dynamic-library/symbol-loading
failure (e.g. `Symbol not found: _XPCTypeBool` from
`libxcodebuildLoader.dylib`), or the component download fails with
`Failed fetching catalog for assetType (com.apple.MobileAsset.MetalToolchain)`,
that's a broken or version-mismatched Xcode install, not something specific
to this project. Fall back to Option B rather than chasing it — reinstalling
Xcode from scratch is the usual fix if you want to come back to Option A
later.

## Option B — from the devcontainer, forwarded to macOS via XQuartz

Confirms the Linux backend (`gpui_linux`) instead, without leaving the
container. Confirmed working end-to-end (2026-09-03, the three Cargo
examples; 2026-09-05, both Vue ports, including a real click updating the
label). Note this is distinct from an agent's headless `Xvfb` attempt from
inside the container (no XQuartz), which rendered nothing — not even the
background — for a reason not yet found.

1. Install XQuartz on the Mac host (not inside the container):
   ```sh
   brew install --cask xquartz
   ```
   After a fresh install, log out and back in — this is a known Homebrew
   caveat, without it XQuartz doesn't finish registering itself.
2. Launch XQuartz. It has no window of its own to open — look for an "X"
   icon in the menu bar to confirm it's running.
3. From that menu bar icon, open Settings → Security, and check "Allow
   connections from network clients." Quit and relaunch XQuartz for this to
   take effect.
4. On the Mac host (not inside the container), allow incoming connections.
   Prefer scoping this to loopback rather than opening it to any host:
   ```sh
   xhost +127.0.0.1
   ```
   This was confirmed to work through Docker Desktop for Mac's networking
   (the container's connection to `host.docker.internal` reaches XQuartz as
   if from `127.0.0.1`). If it doesn't work in your setup, the wider
   `xhost +` (revert with `xhost -` once done) is the fallback.
5. Inside the devcontainer, for examples:
   ```sh
   DISPLAY=host.docker.internal:0 cargo run -p inca-gpui --example hello_world
   ```
   or, for one of the Vue ports (build `inca-host` first, same as
   Option A):
   ```sh
   DISPLAY=host.docker.internal:0 pnpm --filter hello_world dev
   ```

The same window appears on the Mac desktop, rendered by XQuartz.

## Why an agent can't just do this

The devcontainer has no display attached (see
[.devcontainer/devcontainer.json](../.devcontainer/devcontainer.json) — no
X11/Wayland socket is mounted in). An agent working inside it can run the
example under a headless `Xvfb` and confirm the process doesn't crash and a
correctly sized/positioned window gets created (e.g. via `xwininfo`) — but
not that pixel content actually renders (see the note under Option B).
Confirming that needs a real compositor and a human looking at it.
