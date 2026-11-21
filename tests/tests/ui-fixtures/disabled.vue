<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const ptr = ref("");
const mouse = ref("");
const menu = ref("");
const hover = ref("");
const active = ref("");

const token = (e, id) => `${e.type}@${id}(${e.target.id}) `;
const listeners = (id) => ({
  pointerdown: (e) => (ptr.value += token(e, id)),
  pointerup: (e) => (ptr.value += token(e, id)),
  mousedown: (e) => (mouse.value += token(e, id)),
  mouseup: (e) => (mouse.value += token(e, id)),
  click: (e) => (mouse.value += token(e, id)),
  dblclick: (e) => (mouse.value += token(e, id)),
  contextmenu: (e) => (menu.value += token(e, id)),
  auxclick: (e) => (menu.value += token(e, id)),
  mouseover: (e) => (hover.value += token(e, id)),
  mouseenter: (e) => (hover.value += token(e, id)),
  mousemove: (e) => (hover.value += token(e, id)),
});
const read = () => {
  active.value = document.activeElement.id;
};
</script>

<template>
  <div :style="{ width: 400, height: 300 }">
    <div
      id="wrap"
      tabindex="0"
      :style="{ display: 'flex', flexDirection: 'row', width: 360, height: 40 }"
      v-on="listeners('wrap')"
    >
      <button id="off" disabled :style="{ width: 100, height: 40 }" v-on="listeners('off')">
        <div id="kid" :style="{ width: 30, height: 20 }" v-on="listeners('kid')" />
      </button>
      <input
        id="off-input"
        disabled
        :style="{ width: 100, height: 40 }"
        v-on="listeners('off-input')"
      />
      <button id="other" :style="{ width: 100, height: 40 }">other</button>
    </div>
    <button id="off-alone" disabled :style="{ width: 100, height: 40 }">alone</button>
    <div id="read" :style="{ width: 100, height: 40 }" @mousedown.prevent="read" />
    <div id="ptr">{{ ptr }}</div>
    <div id="mouse">{{ mouse }}</div>
    <div id="menu">{{ menu }}</div>
    <div id="hover">{{ hover }}</div>
    <div id="active">{{ active }}</div>
  </div>
</template>
