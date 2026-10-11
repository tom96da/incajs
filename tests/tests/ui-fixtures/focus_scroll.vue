<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const box = ref(null);
const rows = ref([]);
const indexes = Array.from({ length: 30 }, (_, i) => i);
const focused = ref(-1);
const scrollTop = ref(0);

function readScrollTop() {
  scrollTop.value = box.value.scrollTop;
}

function focusTwenty() {
  rows.value[20].focus();
  readScrollTop();
}

function focusTwentyPreventScroll() {
  rows.value[20].focus({ preventScroll: true });
  readScrollTop();
}

function scrollIntoViewTwenty() {
  rows.value[20].scrollIntoView();
  readScrollTop();
}
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'column', width: 300, height: 400, padding: 10 }">
    <!-- 200x100 content box inside 10px padding and 3px border -->
    <div
      id="box"
      ref="box"
      :style="{ width: 226, height: 126, padding: 10, border_width: 3, overflow: 'scroll' }"
      @scroll="readScrollTop"
    >
      <div
        v-for="i in indexes"
        :id="`item-${i}`"
        :key="i"
        ref="rows"
        :tabindex="0"
        :style="{ width: 200, height: 30 }"
        @focus="focused = i"
      />
    </div>
    <div id="focus20" :style="{ width: 100, height: 20 }" @mousedown.prevent @click="focusTwenty" />
    <div
      id="prevent20"
      :style="{ width: 100, height: 20 }"
      @mousedown.prevent
      @click="focusTwentyPreventScroll"
    />
    <div
      id="into20"
      :style="{ width: 100, height: 20 }"
      @mousedown.prevent
      @click="scrollIntoViewTwenty"
    />
    <div id="scrolltop">{{ scrollTop }}</div>
    <div id="focused">{{ focused }}</div>
  </div>
</template>
