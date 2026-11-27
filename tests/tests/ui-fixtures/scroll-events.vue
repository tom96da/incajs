<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "vue";

const box = ref(null);
const log = ref("");
const bub = ref("");

function add(token) {
  log.value = log.value ? `${log.value} ${token}` : token;
}

function onScroll() {
  add(`scroll:${box.value.scrollTop}`);
}

function capture(e) {
  add(`capture@outer(${e.target.id})`);
}

function bubble(e) {
  bub.value = bub.value ? `${bub.value} ` : "";
  bub.value += `bubble@outer(${e.target.id})`;
}

function set40() {
  add("write");
  box.value.scrollTop = 40;
}

function set40Twice() {
  add("write-1");
  box.value.scrollTop = 40;
  add("write-2");
  box.value.scrollTop = 40;
}

function set999() {
  add("write");
  box.value.scrollTop = 999;
}
</script>

<template>
  <div id="outer" @scroll.capture="capture" @scroll="bubble">
    <div
      id="box"
      ref="box"
      :style="{ width: 100, height: 100, overflow: 'auto' }"
      @scroll="onScroll"
    >
      <div v-for="n in 3" :key="n" :style="{ width: 80, height: 60 }" />
    </div>
  </div>
  <div id="set40" :style="{ width: 100, height: 20 }" @mousedown.prevent @click="set40" />
  <div
    id="set40-twice"
    :style="{ width: 100, height: 20 }"
    @mousedown.prevent
    @click="set40Twice"
  />
  <div id="set999" :style="{ width: 100, height: 20 }" @mousedown.prevent @click="set999" />
  <div id="log">{{ log }}</div>
  <div id="bub">{{ bub }}</div>
</template>
