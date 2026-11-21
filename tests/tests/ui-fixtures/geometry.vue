<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

const outer = ref(null);
const inner = ref(null);
const probe = ref(null);
const rect = ref("");
const offset = ref("");
const client = ref("");
const parent = ref("");
const mounted = ref("");

onMounted(() => {
  const el = probe.value;
  mounted.value = `${el.getBoundingClientRect().width},${el.offsetWidth},${el.offsetParent}`;
});

function measure() {
  const r = inner.value.getBoundingClientRect();
  rect.value = JSON.stringify({ x: r.x, y: r.y, width: r.width, height: r.height });
  offset.value = `${inner.value.offsetWidth},${inner.value.offsetHeight},${inner.value.offsetLeft},${inner.value.offsetTop}`;
  client.value = `${inner.value.clientWidth},${inner.value.clientHeight}`;
  parent.value = String(inner.value.offsetParent === outer.value);
}
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'column', width: 300, height: 400, padding: 20 }">
    <div id="outer" ref="outer" :style="{ width: 200, height: 100, padding: 10 }">
      <div id="inner" ref="inner" :style="{ width: 50, height: 30, border_width: 2 }" />
    </div>
    <div id="probe" ref="probe" :style="{ width: 40, height: 20 }" />
    <div id="measure" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="measure" />
    <div id="rect">{{ rect }}</div>
    <div id="offset">{{ offset }}</div>
    <div id="client">{{ client }}</div>
    <div id="parent">{{ parent }}</div>
    <div id="mounted">{{ mounted }}</div>
  </div>
</template>
