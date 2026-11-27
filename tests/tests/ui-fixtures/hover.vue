<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "vue";

const text = ref("");
const hasC = ref(true);
const hasD = ref(true);

function log(e) {
  const related = e.relatedTarget ? e.relatedTarget.id || "-" : "null";
  text.value += `${e.type}@${e.currentTarget.id}(${e.target.id})<${related} `;
}
</script>

<template>
  <div
    id="home"
    :style="{ width: 100, height: 20 }"
    @mouseover="log"
    @mouseout="log"
    @mouseenter="log"
    @mouseleave="log"
  />
  <div
    id="a"
    :style="{ width: 200, height: 120, padding: 10 }"
    @mouseover="log"
    @mouseout="log"
    @mouseenter="log"
    @mouseleave="log"
  >
    <div
      id="b"
      :style="{ width: 150, height: 100, padding: 20 }"
      @mouseover="log"
      @mouseout="log"
      @mouseenter="log"
      @mouseleave="log"
    >
      <div
        v-if="hasC"
        id="c"
        :style="{ width: 40, height: 20 }"
        @mousedown="hasC = false"
        @mouseover="log"
        @mouseout="log"
        @mouseenter="log"
        @mouseleave="log"
      />
      <div
        v-if="hasD"
        id="d"
        :style="{ width: 40, height: 20 }"
        @mousedown="hasD = false"
        @mouseover="log"
        @mouseout="log"
        @mouseenter="log"
        @mouseleave="log"
      />
    </div>
  </div>
  <div
    id="e"
    :style="{ width: 100, height: 20 }"
    @mouseover="log"
    @mouseout="log"
    @mouseenter="log"
    @mouseleave="log"
  />
  <div id="log">{{ text }}</div>
</template>
