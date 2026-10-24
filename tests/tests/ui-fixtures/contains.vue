<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { KeepAlive, onMounted, ref } from "@vue/runtime-core";

import { document, window } from "incajs";

import Tally from "./parts/tally.vue";

const log = ref("");
const order = ref(["a", "b"]);
const shown = ref(true);
const kept = ref(true);

const guard = (e) => {
  const path = e.composedPath();
  const chain = [];
  for (let n = e.target; n !== null; n = n.parent) chain.push(n);
  const held =
    path.length === chain.length + 2 &&
    chain.every((n, i) => n === path[i]) &&
    path.at(-2) === document &&
    path.at(-1) === window &&
    chain.every((n) => n.contains(e.target));
  log.value += `${e.target.id}:${held} `;
};
onMounted(() => window.addEventListener("click", guard, true));
</script>

<template>
  <div id="app" :style="{ width: 300, height: 400 }">
    <div :style="{ width: 200, height: 40 }">
      <div :style="{ width: 150, height: 30 }">
        <div id="leaf" :style="{ width: 100, height: 20 }" />
      </div>
    </div>
    <div id="reverse" :style="{ width: 100, height: 20 }" @click="order.reverse()" />
    <div v-for="key in order" :id="key" :key="key" :style="{ width: 100, height: 20 }" />
    <div id="hide" :style="{ width: 100, height: 20 }" @click="shown = false" />
    <div v-if="shown" :style="{ width: 100, height: 20 }">
      <div :style="{ width: 50, height: 10 }" />
    </div>
    <div id="after" :style="{ width: 100, height: 20 }" />
    <div id="switch" :style="{ width: 100, height: 20 }" @click="kept = !kept" />
    <KeepAlive>
      <Tally v-if="kept" />
      <div v-else id="other" :style="{ width: 100, height: 20 }" />
    </KeepAlive>
    <div id="log" :style="{ width: 300, height: 20 }">{{ log }}</div>
  </div>
</template>
