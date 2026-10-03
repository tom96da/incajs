---
description: What's shipped, in progress, and planned for inca, phase by phase.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Roadmap

**Incarnative.js** is built in phases, Vue 3 support first and end-to-end, before the
surface broadens to more styling, tooling, platforms, and a second
frontend framework. This page summarizes where each phase stands.

::::: roadmap

:::: shipped Phase 1: Rust host & FFI bridge core
The QuickJS runtime, the retained render tree, and the GPUI rendering pipeline underneath every app.
::::

:::: shipped Phase 2: JS core bridge & Vue 3 custom renderer
incajs and incajs/vue, a custom Vue 3 renderer.
::::

:::: shipped Phase 3.1–3.3: Developer tooling & packaging
inca dev, inca build, and inca package, with per-platform prebuilt host binaries. This is the v0.0.1 release milestone. The current bundler adapter is Vite; a future adapter/rspack would sit alongside it as a sibling module.
::::

:::: in-progress Phase 3.4: HMR
Functional behind an opt-in flag (--experimental-hmr / INCA_EXPERIMENTAL_HMR). Plain inca dev still does a full reload.

[HMR guide](/guide/hmr)
::::

:::: in-progress Phase 4: Input & text editing
Pointer and keyboard input are done. Scrolling is done. Text editing (selection, caret, IME composition) is not built yet.

[Elements & Styles](/reference/elements)
::::

:::: not-started Phase 5: Accessibility
A semantics layer, platform accessibility APIs, and keyboard reachability. Waits on the input/focus model above being stable.
::::

:::: not-started Phase 6: Runtime standard library
Timers, fetch/WebSocket, filesystem access, and engine resource limits.
::::

:::: not-started Phase 7: Majority style & Tailwind coverage
Most layout, spacing, and typography utilities, mapped directly onto native styles.
::::

:::: not-started Phase 8: Developer tools
The real Vue DevTools running against an app.
::::

:::: not-started Phase 9: Application shell & platform integration
Multiple windows, native menus beyond the current Quit item, dialogs, clipboard, notifications.
::::

:::: not-started Phase 10: React custom renderer
incajs/react alongside incajs/vue.
::::

:::: not-started Phase 11: Cross-platform support
Confirmed Linux rendering and a Windows backend.
::::

:::: not-started Phase 12: 100% style & Tailwind parity
State variants, responsive breakpoints, dark mode, and the remaining CSS surface.
::::

:::: not-started Phase 13: App-owned Rust extensions
Apps linking their own Rust code into the host binary.
::::
:::::
