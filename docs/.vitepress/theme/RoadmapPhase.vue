<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup lang="ts">
type Status = "shipped" | "in-progress" | "not-started";

defineProps<{ status: Status; id: string; title: string }>();

const statusLabel: Record<Status, string> = {
  shipped: "Shipped",
  "in-progress": "In progress",
  "not-started": "Not started",
};
</script>

<template>
  <li :class="status">
    <span class="phase-id">Phase {{ id }}</span>
    <h3 class="phase-title">{{ title }}</h3>
    <span class="phase-status">{{ statusLabel[status] }}</span>
    <div class="phase-description"><slot /></div>
  </li>
</template>

<style scoped>
.phase-id {
  display: block;
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--vp-c-text-2);
}

.phase-title {
  margin: 0.1rem 0 0.3rem;
  font-size: 1.05rem;
  border-top: none;
}

.phase-status {
  display: inline-block;
  margin-bottom: 0.4rem;
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.02em;
  color: var(--vp-c-text-2);
}

.phase-description :deep(p) {
  margin: 0.2rem 0;
}
</style>
