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

Run with no arguments, it looks for `bundle.js` beside its own executable,
then beside it in `../Resources/` (a macOS `.app`'s layout) — the search a
packaged app's launcher relies on, since a double-clicked `.app` gets no
argv and an unpredictable working directory.

It loads that one entry file, resolving any `import` in it against its
own directory on disk — no knowledge of Vite, dev servers, or HMR, and
never watches for changes. Rebuild the bundle and re-run it to pick up
an edit.
