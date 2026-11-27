<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "vue";

const indexes = Array.from({ length: 10 }, (_, i) => i);
const port = ref(null);
const items = ref([]);
const hItems = ref([]);

const actions = {
  "si-default": () => items.value[5].scrollIntoView(),
  "si-start": () => items.value[5].scrollIntoView({ block: "start" }),
  "si-center": () => items.value[5].scrollIntoView({ block: "center" }),
  "si-end": () => items.value[5].scrollIntoView({ block: "end" }),
  "si-nearest": () => items.value[5].scrollIntoView({ block: "nearest" }),
  "jump-200": () => (port.value.scrollTop = 200),
  "jump-10": () => (port.value.scrollTop = 10),
  "jump-20": () => (port.value.scrollTop = 20),
  "si-nearest-2": () => items.value[2].scrollIntoView({ block: "nearest" }),
  "si-nearest-1": () => items.value[1].scrollIntoView({ block: "nearest" }),
  "si-start-9": () => items.value[9].scrollIntoView({ block: "start" }),
  "append-reveal": () => {
    const extra = document.createElement("div");
    extra.id = "extra";
    extra.style.width = "80px";
    extra.style.height = "30px";
    port.value.appendChild(extra);
    extra.scrollIntoView();
  },
  "focus-5": () => items.value[5].focus(),
  "focus-3": () => items.value[3].focus(),
  "focus-0": () => items.value[0].focus(),
  "focus-1": () => items.value[1].focus(),
  "focus-5-prevent": () => items.value[5].focus({ preventScroll: true }),
  "si-inline-center": () => hItems.value[5].scrollIntoView({ inline: "center" }),
  "si-inline-end": () => hItems.value[5].scrollIntoView({ inline: "end" }),
  "si-true": () => items.value[5].scrollIntoView(true),
  "si-false": () => items.value[5].scrollIntoView(false),
};
const triggers = Object.keys(actions);
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'column', width: 300, height: 400 }">
    <div id="port" ref="port" :style="{ width: 100, height: 100, overflow: 'auto' }">
      <div
        v-for="i in indexes"
        :id="`item-${i}`"
        :key="i"
        ref="items"
        :tabindex="0"
        :style="{ width: 80, height: 30 }"
      />
    </div>
    <div
      id="hport"
      :style="{ display: 'flex', flex_direction: 'row', width: 100, height: 100, overflow: 'auto' }"
    >
      <div
        v-for="i in indexes"
        :id="`h-${i}`"
        :key="i"
        ref="hItems"
        :style="{ width: 80, height: 30, flex_shrink: 0 }"
      />
    </div>
    <div :style="{ display: 'flex', flex_direction: 'row' }">
      <div
        v-for="t in triggers"
        :id="t"
        :key="t"
        :style="{ width: 12, height: 12 }"
        @mousedown.prevent
        @click="actions[t]"
      />
    </div>
  </div>
</template>
