<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

import { document } from "incajs";

const host = ref(null);
const list = ref(null);
const model = ref(null);
const outer = ref(null);
const inner = ref(null);
const sibling = ref(null);
const old = ref(null);
const word = ref("w");
const cloneLog = ref("");
const containsLog = ref("");
const positionLog = ref("");

const replaceText = () => {
  old.value = host.value.firstChild;
  host.value.textContent = "x";
};
const reuseText = () => {
  old.value.data = "z";
  host.value.appendChild(old.value);
};
const fillList = () => {
  const fragment = document.createDocumentFragment();
  for (const id of ["c1", "c2", "c3"]) {
    const clone = model.value.cloneNode(true);
    clone.id = id;
    fragment.appendChild(clone);
  }
  list.value.appendChild(fragment);
};
const compare = () => {
  const clone = outer.value.cloneNode(true);
  cloneLog.value = [
    clone.isEqualNode(outer.value),
    clone.isEqualNode(sibling.value),
    clone.firstChild.isEqualNode(inner.value),
  ].join(" ");
  containsLog.value = [
    outer.value.contains(inner.value),
    inner.value.contains(outer.value),
    outer.value.contains(outer.value),
    outer.value.contains(sibling.value),
  ].join(" ");
  positionLog.value = [
    outer.value.compareDocumentPosition(sibling.value),
    sibling.value.compareDocumentPosition(outer.value),
    outer.value.compareDocumentPosition(inner.value),
    inner.value.compareDocumentPosition(outer.value),
  ].join(" ");
};
</script>

<template>
  <div :style="{ width: 300, height: 400 }">
    <div id="replace" :style="{ width: 100, height: 20 }" @click="replaceText" />
    <div id="reuse" :style="{ width: 100, height: 20 }" @click="reuseText" />
    <div id="fill" :style="{ width: 100, height: 20 }" @click="fillList" />
    <div id="compare" :style="{ width: 100, height: 20 }" @click="compare" />
    <div id="host" ref="host" :style="{ width: 100, height: 20 }">
      {{ word }}
      <div :style="{ width: 10, height: 10 }" />
    </div>
    <div id="list" ref="list" :style="{ width: 100 }" />
    <div id="model" ref="model" :style="{ width: 100, height: 10 }" />
    <div ref="outer" :style="{ width: 100, height: 20 }">
      <div ref="inner" :style="{ width: 50, height: 10 }" />
    </div>
    <div ref="sibling" :style="{ width: 100, height: 20 }" />
    <div id="clone-log" :style="{ width: 300, height: 20 }">{{ cloneLog }}</div>
    <div id="contains-log" :style="{ width: 300, height: 20 }">{{ containsLog }}</div>
    <div id="position-log" :style="{ width: 300, height: 20 }">{{ positionLog }}</div>
  </div>
</template>
