<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-host

The runtime binary behind an inca app: given the path to a prebuilt JS
entry file, it installs `__inca_native__` bindings and a `console`,
evaluates the entry as an ES module, and opens a GPUI window rendering
whatever tree it mounted. The app's `console` output goes to stderr.

```sh
cargo run -p inca-host -- path/to/bundle.js
```

Beside that entry it reads `inca.json`, if one is there: the name the
platform calls the app, its reverse-DNS identifier, and the size and title
its window opens at. Anything missing falls back to a default.

The window carries an application menu with a `Quit` item, bound to `cmd-q`
on macOS and `ctrl-q` elsewhere. Only macOS draws a menu bar for it.

Run with no arguments, it looks for `bundle.js` beside its own executable,
then beside it in `../Resources/` (a macOS `.app`'s layout) — the search a
packaged app's launcher relies on, since a double-clicked `.app` gets no
argv and an unpredictable working directory.

It loads that one entry file, resolving any `import` in it against its
own directory on disk — no knowledge of Vite, dev servers, or HMR, and
never watches for changes. Rebuild the bundle and re-run it to pick up
an edit.

With `--dev` the host also reads JSON-RPC messages on stdin and writes its
own on stdout. It starts the entry and serves those messages while the entry's
top-level `await` is pending. The window opens at the entry's content size once
the `await` settles. An entry still pending after 2 seconds opens the window at
the size the config gives, else the default size, unless the app reported a
failed load with `__inca_dev__.setLoadFailed(true)`. The host then keeps waiting until a load
succeeds. A new attempt reported by the app restarts the 2 seconds.

`--print-config` reads the `inca.json` beside the entry, prints the settings
the host applies as JSON, and returns after printing. A `null` marks a value
the app leaves open: its root element sizes the window, and an unset limit
stays open. Given no path, it prints the config of the `bundle.js` found by
that search. It exits 0 after printing and 1 when the path is not a file.

```sh
cargo run -p inca-host -- --print-config path/to/bundle.js
```
