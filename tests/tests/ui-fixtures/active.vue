<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";
import { activeElement } from "incajs/vue";

const a = ref(null);
const b = ref(null);
const n = ref(null);
const clickTarget = ref(null);
const doomed = ref(null);
const alive = ref(true);
const result = ref("");
const result2 = ref("");
const focusResult = ref("");

function record(value) {
  result.value = String(value);
}

function focusA() {
  a.value.focus();
  record(activeElement() === a.value);
}

function focusB() {
  b.value.focus();
  record(activeElement() === b.value);
}

function blurA() {
  a.value.blur();
  record(activeElement() === null);
}

function focusN() {
  n.value.focus();
  record(activeElement() === null);
}

function onFocusB() {
  focusResult.value = String(activeElement() === b.value);
}

function onClickTarget() {
  record(activeElement() === clickTarget.value);
}

function kill() {
  alive.value = false;
}

function probeDoomed() {
  result2.value = String(activeElement() === null);
}

function probeDoomedFocused() {
  record(activeElement() === doomed.value);
}
</script>

<template>
  <div :style="{ width: 300, height: 800 }">
    <div id="a" ref="a" tabindex="0" :style="{ width: 100, height: 40 }" />
    <div id="b" ref="b" tabindex="0" :style="{ width: 100, height: 40 }" @focus="onFocusB" />
    <div id="not-focusable" ref="n" :style="{ width: 100, height: 40 }" />
    <div id="focus-a" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="focusA" />
    <div id="focus-b" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="focusB" />
    <div id="blur-a" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="blurA" />
    <div id="focus-n" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="focusN" />
    <div
      id="click-target"
      ref="clickTarget"
      tabindex="0"
      :style="{ width: 100, height: 40 }"
      @click="onClickTarget"
    />
    <div v-if="alive" id="doomed" ref="doomed" tabindex="0" :style="{ width: 100, height: 40 }" />
    <div id="kill" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="kill" />
    <div
      id="probe-doomed"
      :style="{ width: 100, height: 40 }"
      @mousedown.prevent
      @click="probeDoomed"
    />
    <div
      id="probe-doomed-focused"
      :style="{ width: 100, height: 40 }"
      @mousedown.prevent
      @click="probeDoomedFocused"
    />
    <div id="result">{{ result }}</div>
    <div id="result2">{{ result2 }}</div>
    <div id="focus-result">{{ focusResult }}</div>
  </div>
</template>
