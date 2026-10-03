// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { container } from "@mdit/plugin-container";

type Markdown = Parameters<typeof container>[0];

const statuses = ["shipped", "in-progress", "not-started"];

/**
 * The `roadmap` container wraps `RoadmapPhase` containers, one per status:
 *
 *     ::::: roadmap
 *     :::: shipped Phase 1: Title
 *     Description in markdown.
 *     ::::
 *     :::::
 *
 * They render as `RoadmapTimeline` and `RoadmapPhase`, and the markdown
 * stays in the page source for readers other than the browser.
 */
export function roadmapContainers(md: Markdown) {
  md.use(container, {
    name: "roadmap",
    openRenderer: () => "<RoadmapTimeline>\n",
    closeRenderer: () => "</RoadmapTimeline>\n",
  });

  for (const status of statuses) {
    md.use(container, {
      name: status,
      openRenderer: (tokens, index) => {
        const params = (tokens[index]?.info ?? "").trim().slice(status.length).trim();
        const [, id = "", title = params] = params.match(/^Phase ([^:]+):\s*(.*)$/) ?? [];
        const escape = md.utils.escapeHtml;
        return `<RoadmapPhase status="${status}" id="${escape(id)}" title="${escape(title)}">\n`;
      },
      closeRenderer: () => "</RoadmapPhase>\n",
    });
  }
}
