<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, reactive, ref } from "@vue/runtime-core";

import { window } from "incajs";

const log = ref("");
const outer = ref(null);
const inner = ref(null);
const holder = reactive({ el: null });
let kept = null;

const name = (node) => (node === window ? "window" : node === null ? "null" : node.id);
const note = (key, e) => {
  const path = e.composedPath().map(name).join(",");
  log.value += `${key}:${name(e.target)}>${name(e.currentTarget)}[${path}] `;
};
const ping = () => inner.value.dispatchEvent(new Event("ping", { bubbles: true }));
const after = () => {
  const length = kept.composedPath().length;
  log.value += `after:${name(kept.target)}>${name(kept.currentTarget)}[${length}] `;
};
const ids = (e) => {
  log.value += `ids:${JSON.stringify(e.target.id)}>${JSON.stringify(e.currentTarget.id)} `;
};

onMounted(() => {
  window.addEventListener("keydown", (e) => note("key", e));
  inner.value.addEventListener("ping", (e) => note("ping", e));
  outer.value.addEventListener("click", function (e) {
    note("click", e);
    holder.el = e.target;
    log.value +=
      `this:${this === e.currentTarget} ref:${e.target === inner.value} ` +
      `reactive:${holder.el === inner.value} `;
    kept = e;
  });
});
</script>

<template>
  <div id="app" :style="{ width: 300, height: 400 }">
    <div id="outer" ref="outer" :style="{ width: 200, height: 80 }">
      <div id="inner" ref="inner" tabindex="0" :style="{ width: 100, height: 40 }" />
    </div>
    <div id="fire" :style="{ width: 100, height: 20 }" @click="inner.click()" />
    <div id="after" :style="{ width: 100, height: 20 }" @click="after" />
    <div id="ping" :style="{ width: 100, height: 20 }" @click="ping" />
    <div id="focus" :style="{ width: 100, height: 20 }" @click="inner.focus()" />
    <div :style="{ width: 100, height: 40 }" @click="ids">
      <div id="leaf" :style="{ width: 50, height: 20 }" />
    </div>
    <div id="log">{{ log }}</div>
  </div>
</template>
