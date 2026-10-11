<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const show = ref(true);
const result = ref("");
let stale = null;

function keep(el) {
  if (el) stale = el;
}

function poke() {
  try {
    stale.addEventListener("click", () => {});
    const tabIndex = stale.tabIndex;
    stale.tabIndex = 5;
    const dispatched = stale.dispatchEvent(new Event("ping"));
    result.value = `ok:${tabIndex}:${dispatched}`;
  } catch (error) {
    result.value = `threw:${error.name}`;
  }
}
</script>

<template>
  <div :style="{ width: 300, height: 400 }">
    <button v-if="show" id="target" :ref="keep" tabindex="2" :style="{ width: 100, height: 40 }" />
    <div id="remove" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="show = false" />
    <div id="poke" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="poke" />
    <div id="result">{{ result }}</div>
  </div>
</template>
