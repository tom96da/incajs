<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<template>
  <ol class="roadmap-timeline">
    <slot />
  </ol>
</template>

<style scoped>
.roadmap-timeline {
  --line-width: 2px;
  --line-x: 0.3rem;
  --dot-size: 0.65rem;
  position: relative;
  margin: 2rem 0;
  padding: 0;
  list-style: none;
}

.roadmap-timeline::before {
  content: "";
  position: absolute;
  top: 0.4rem;
  bottom: 0.4rem;
  left: var(--line-x);
  width: var(--line-width);
  background: var(--vp-c-divider);
}

.roadmap-timeline > :deep(li) {
  position: relative;
  padding: 0 0 1.75rem 1.75rem;
}

.roadmap-timeline > :deep(li:last-child) {
  padding-bottom: 0;
}

.roadmap-timeline > :deep(li::before) {
  content: "";
  position: absolute;
  top: 0.3rem;
  left: calc(var(--line-x) + var(--line-width) / 2 - var(--dot-size) / 2);
  width: var(--dot-size);
  height: var(--dot-size);
  border-radius: 50%;
  box-shadow: 0 0 0 3px var(--vp-c-bg);
}

.roadmap-timeline > :deep(li.shipped::before) {
  background: var(--vp-c-success-1);
}

.roadmap-timeline > :deep(li.not-started::before) {
  background: transparent;
  border: 2px solid var(--vp-c-default-3);
}

.roadmap-timeline > :deep(li.in-progress::before) {
  background: var(--vp-c-warning-1);
}

@media (prefers-reduced-motion: no-preference) {
  .roadmap-timeline > :deep(li.in-progress::before) {
    animation: roadmap-pulse 2s ease-in-out infinite;
  }
}

@keyframes roadmap-pulse {
  0%,
  100% {
    box-shadow: 0 0 0 3px var(--vp-c-bg);
  }
  50% {
    box-shadow:
      0 0 0 3px var(--vp-c-bg),
      0 0 0 6px var(--vp-c-warning-soft);
  }
}
</style>
