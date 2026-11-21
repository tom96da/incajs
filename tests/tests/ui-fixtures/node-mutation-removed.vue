<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const show = ref(true);
const result = ref("");
const dispatch = ref("");
let stale = null;
let heard = false;

function keep(el) {
  if (!el) return;
  stale = el;
  el.addEventListener("ping", () => {
    heard = true;
  });
}

function poke() {
  try {
    stale.addEventListener("click", () => {});
    const id = stale.id;
    const tabIndex = stale.tabIndex;
    stale.tabIndex = 5;
    const attribute = stale.getAttribute("tabindex");
    const dispatched = stale.dispatchEvent(new Event("ping"));
    result.value = `ok:${id}:${tabIndex}:${attribute}`;
    dispatch.value = `${dispatched}:${heard}`;
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
    <div id="dispatch">{{ dispatch }}</div>
  </div>
</template>
