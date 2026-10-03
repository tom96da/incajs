<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Manually verifying a GPUI window opens

How to run this repo's examples and look at the window. The devcontainer
mounts no X11 or Wayland socket, so window checks run on a host with a
display. Under a headless `Xvfb` the window gets its size (`xwininfo` shows
it), but nothing renders into it.

Option A is enough for most checks. Option B exercises the Linux backend.

## Option A — natively on macOS (recommended)

Run these on a macOS checkout, against the macOS backend.

```sh
cargo run -p inca-gpui --example gpui_hello_world
```

A window with a gray background, the text "Hello, World!", and a row of six
colored boxes. The other Cargo examples are `hello_world` and
`click_counter`.

### The Vue ports

```sh
cargo build -p inca-host
INCA_HOST_BIN="$(pwd)/target/debug/inca-host" pnpm --filter hello_world dev
```

Same look as above. `click_counter` instead gives a clickable box that
counts up.

### The app's name, icon, window and menu (macOS)

Same dev run. `inca dev` assembles `node_modules/.inca/<productName>.app`
and launches the host from inside it.

1. The Dock tile and the menu bar carry the app's `productName`, and the
   Dock its `icon` where the config declares one.
2. The window opens at the `window.width`/`window.height` the config
   declares, and its title bar carries `window.title`.
3. The application menu has a `Quit` item with `⌘Q` beside it.
4. `⌘Q` quits, and `inca dev` stops with it.

### A packaged app

```sh
cargo build -p inca-host --release
pnpm --filter click_counter package
open examples/click_counter/dist/click_counter.app
```

Same look and click behavior as the dev run. Double-click it from Finder to
also confirm it starts with no terminal attached.

### Metal toolchain

`gpui_apple`'s build script needs the `metal` shader compiler. Xcode 26 and
later do not bundle it. It is a separate component of the full Xcode.app,
which the Command Line Tools do not provide.

#### Install

1. Install Xcode from the App Store.
2. Point the active developer directory at it:
   ```sh
   sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
   sudo xcodebuild -license accept
   ```
3. Download the toolchain component (about 688 MB):
   ```sh
   xcodebuild -downloadComponent MetalToolchain
   ```
   Xcode's Settings → Components installs it too.
4. Confirm that `xcrun -sdk macosx metal --version` prints a version line.

On macOS 15 (Sequoia) and later, step 3 can download but fail to install.
Use the manual install below.

#### Manual install (macOS 15 and later)

SIP blocks the automatic install. Copy the toolchain by hand:

```sh
# 1. Find the downloaded DMG (its directory name is a hash)
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

#### Errors

| Message | Cause | Fix |
| --- | --- | --- |
| `error: cannot execute tool 'metal' due to missing Metal Toolchain; use: xcodebuild -downloadComponent MetalToolchain` | the Metal Toolchain component is not installed | Install, step 3 |
| `xcrun: error: unable to find utility "metal", not a developer tool or in PATH` | Xcode.app or the developer directory is missing, or the component is not installed | Install, steps 1-2 |
| `Metal Toolchain unable to refresh cache with error: … "Operation not permitted"` | SIP blocks the automatic install | Manual install |
| `Symbol not found: _XPCTypeBool` from `libxcodebuildLoader.dylib`, or `Failed fetching catalog for assetType (com.apple.MobileAsset.MetalToolchain)` | the Xcode install is broken | Reinstall Xcode, or use Option B |

## Option B — devcontainer, forwarded to macOS via XQuartz

Exercises `gpui_linux` without leaving the container.

1. On the Mac host, install XQuartz, then log out and back in. XQuartz does
   not finish registering itself without that:
   ```sh
   brew install --cask xquartz
   ```
2. Launch XQuartz and look for its "X" icon in the menu bar.
3. From that icon, open Settings → Security and check "Allow connections
   from network clients". Quit and relaunch XQuartz.
4. On the Mac host, allow incoming connections from loopback — Docker
   Desktop's networking makes the container reach XQuartz as `127.0.0.1`:
   ```sh
   xhost +127.0.0.1
   ```
   `xhost +` also works, and `xhost -` reverts it.
5. Inside the devcontainer:
   ```sh
   DISPLAY=host.docker.internal:0 cargo run -p inca-gpui --example hello_world
   ```
   or, for a Vue port (build `inca-host` first, as in Option A):
   ```sh
   DISPLAY=host.docker.internal:0 pnpm --filter hello_world dev
   ```

The window appears on the Mac desktop, rendered by XQuartz.
