<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# `patches/`

Local crates `[patch]`ed into the build via root `Cargo.toml`, replacing
upstream crates this workspace never wants to actually compile — not
`third_party/`, which holds real upstream sources kept for reference.

- `ztracing`, `ztracing_macro` — no-op stand-ins for zed-industries/zed's
  crates of the same name, which `gpui` depends on. Avoids pulling in
  their GPL-3.0-or-later license (`gpui` itself is Apache-2.0). Written
  from the public API contract, not copied from the originals.
