<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "vue";

const text = ref("");
const mods = ref("");
const focus = ref("");
const active = ref("");
const types = [
  "pointerdown",
  "mousedown",
  "pointerup",
  "mouseup",
  "click",
  "dblclick",
  "contextmenu",
  "auxclick",
];
const flagged = new Set(["pointerdown", "mousedown", "pointerup", "mouseup", "click"]);

function listeners(id) {
  const out = {};
  for (const type of types) {
    out[type] = (e) => {
      text.value += `${id}:${type}:${e.detail}:${e.button}:${e.buttons} `;
      if (id === "box" && flagged.has(type)) {
        mods.value += `${type}:${e.ctrlKey}:${e.altKey}:${e.shiftKey}:${e.metaKey} `;
      }
    };
  }
  return out;
}

function read() {
  active.value = document.activeElement.id;
}
</script>

<template>
  <div :style="{ width: 400, height: 300 }">
    <div id="box" :style="{ width: 100, height: 40 }" v-on="listeners('box')" />
    <div
      id="box2"
      tabindex="0"
      :style="{ width: 100, height: 40 }"
      v-on="listeners('box2')"
      @mousedown.prevent
      @focus="focus += 'box2 '"
    />
    <div
      id="box3"
      tabindex="0"
      :style="{ width: 100, height: 40 }"
      v-on="listeners('box3')"
      @pointerdown.prevent
    />
    <div id="read" :style="{ width: 100, height: 40 }" @mousedown.prevent="read" />
    <div id="log">{{ text }}</div>
    <div id="mods">{{ mods }}</div>
    <div id="focus">{{ focus }}</div>
    <div id="active">{{ active }}</div>
  </div>
</template>
