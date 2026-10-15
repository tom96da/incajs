<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# inca-test-macros

Test attribute macros that the other crates use as a dev-dependency. It is
never part of the `inca-host` build.

`gpui_cases` turns one test function into one test per `#[case]`, next to a
`#[gpui::test]` or `#[test]` attribute. It names each test like `rstest`:
`<fn>::case_<N>_<id>`.
