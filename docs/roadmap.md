---
description: What's shipped, in progress, and planned for inca, phase by phase.
---

<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup lang="ts">
import RoadmapTimeline from "./.vitepress/theme/RoadmapTimeline.vue";

const phases = [
  {
    id: "1",
    title: "Rust host & FFI bridge core",
    status: "shipped",
    description:
      "The QuickJS runtime, the retained render tree, and the GPUI rendering pipeline underneath every app.",
  },
  {
    id: "2",
    title: "JS core bridge & Vue 3 custom renderer",
    status: "shipped",
    description: "incajs and incajs/vue, a custom Vue 3 renderer.",
  },
  {
    id: "3.1–3.3",
    title: "Developer tooling & packaging",
    status: "shipped",
    description:
      "inca dev, inca build, and inca package, with per-platform prebuilt host binaries. This is the v0.0.1 release milestone. The current bundler adapter is Vite; a future adapter/rspack would sit alongside it as a sibling module.",
  },
  {
    id: "3.4",
    title: "HMR",
    status: "in-progress",
    description:
      "Functional behind an opt-in flag (--experimental-hmr / INCA_EXPERIMENTAL_HMR). Plain inca dev still does a full reload.",
    link: { text: "HMR guide", href: "/guide/hmr" },
  },
  {
    id: "4",
    title: "Input & text editing",
    status: "in-progress",
    description:
      "Pointer and keyboard input are done. Text editing (selection, caret, IME composition) and scrolling are not built yet.",
    link: { text: "Elements & Styles", href: "/reference/elements" },
  },
  {
    id: "5",
    title: "Accessibility",
    status: "not-started",
    description:
      "A semantics layer, platform accessibility APIs, and keyboard reachability. Waits on the input/focus model above being stable.",
  },
  {
    id: "6",
    title: "Runtime standard library",
    status: "not-started",
    description:
      "Timers, fetch/WebSocket, filesystem access, and engine resource limits.",
  },
  {
    id: "7",
    title: "Majority style & Tailwind coverage",
    status: "not-started",
    description:
      "Most layout, spacing, and typography utilities, mapped directly onto native styles.",
  },
  {
    id: "8",
    title: "Developer tools",
    status: "not-started",
    description: "The real Vue DevTools running against an app.",
  },
  {
    id: "9",
    title: "Application shell & platform integration",
    status: "not-started",
    description:
      "Multiple windows, native menus beyond the current Quit item, dialogs, clipboard, notifications.",
  },
  {
    id: "10",
    title: "React custom renderer",
    status: "not-started",
    description: "incajs/react alongside incajs/vue.",
  },
  {
    id: "11",
    title: "Cross-platform support",
    status: "not-started",
    description: "Confirmed Linux rendering and a Windows backend.",
  },
  {
    id: "12",
    title: "100% style & Tailwind parity",
    status: "not-started",
    description:
      "State variants, responsive breakpoints, dark mode, and the remaining CSS surface.",
  },
  {
    id: "13",
    title: "App-owned Rust extensions",
    status: "not-started",
    description: "Apps linking their own Rust code into the host binary.",
  },
];
</script>

# Roadmap

inca is built in phases, Vue 3 support first and end-to-end, before the
surface broadens to more styling, tooling, platforms, and a second
frontend framework. This page summarizes where each phase stands.

<RoadmapTimeline :items="phases" />
