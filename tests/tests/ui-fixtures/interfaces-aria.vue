<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "vue";

const result = ref("");
const label = ref("save");
const owner = ref(null);
const tab = ref(null);

function probe() {
  const labelled = owner.value.ariaLabelledByElements;
  result.value = JSON.stringify({
    boundLabel: owner.value.ariaLabel,
    boundLabelAttribute: owner.value.getAttribute("aria-label"),
    role: tab.value.role,
    selected: tab.value.ariaSelected,
    labelledBy: labelled.map((element) => element.id),
  });
}
</script>

<template>
  <div :style="{ width: 300, height: 400 }">
    <div id="name" :style="{ width: 50, height: 20 }" />
    <div ref="owner" :ariaLabel="label" aria-labelledby="name" :style="{ width: 50, height: 20 }" />
    <div ref="tab" role="tab" aria-selected="true" :style="{ width: 50, height: 20 }" />
    <div id="probe" :style="{ width: 50, height: 20 }" @mousedown.prevent @click="probe" />
    <div id="result">{{ result }}</div>
  </div>
</template>
