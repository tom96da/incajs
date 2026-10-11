<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const box = ref(null);
const hidden = ref(null);
const items = ref(["first", "second", "third"]);
const result = ref("");
const scrolls = ref(0);

function scrollBoxTo40() {
  box.value.scrollTop = 40;
}

function scrollHiddenTo40() {
  hidden.value.scrollTop = 40;
}

function append() {
  items.value.push("fourth");
}

function scrollBoxToBottom() {
  box.value.scrollTop = box.value.scrollHeight;
  result.value = `${box.value.scrollTop},${box.value.scrollHeight},${box.value.clientHeight}`;
}
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'column', width: 300, height: 400, padding: 10 }">
    <div
      id="box"
      ref="box"
      @scroll="scrolls++"
      :style="{ width: 120, height: 100, padding: 10, border_width: 3, overflow: 'scroll' }"
    >
      <div v-for="item in items" :id="item" :key="item" :style="{ width: 80, height: 60 }" />
    </div>
    <div
      id="hidden"
      ref="hidden"
      :style="{ width: 120, height: 100, padding: 10, border_width: 3, overflow: 'hidden' }"
    >
      <div id="hidden-first" :style="{ width: 80, height: 60 }" />
      <div id="hidden-second" :style="{ width: 80, height: 60 }" />
    </div>
    <div
      id="scroll40"
      :style="{ width: 100, height: 20 }"
      @mousedown.prevent
      @click="scrollBoxTo40"
    />
    <div
      id="hidden40"
      :style="{ width: 100, height: 20 }"
      @mousedown.prevent
      @click="scrollHiddenTo40"
    />
    <div id="append" :style="{ width: 100, height: 20 }" @mousedown.prevent @click="append" />
    <div
      id="bottom"
      :style="{ width: 100, height: 20 }"
      @mousedown.prevent
      @click="scrollBoxToBottom"
    />
    <div id="result">{{ result }}</div>
    <div id="scrolls">{{ scrolls }}</div>
  </div>
</template>
