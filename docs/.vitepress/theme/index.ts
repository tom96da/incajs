// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import DefaultTheme from "vitepress/theme";
import type { Theme } from "vitepress";

import "@fontsource/outfit/latin-400.css";
import "@fontsource/outfit/latin-500.css";
import "@fontsource/outfit/latin-700.css";
import "virtual:group-icons.css";
import "./styles.css" with { type: "css" };
import RoadmapPhase from "./RoadmapPhase.vue";
import RoadmapTimeline from "./RoadmapTimeline.vue";

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component("RoadmapTimeline", RoadmapTimeline);
    app.component("RoadmapPhase", RoadmapPhase);
  },
} satisfies Theme;
