<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "vue";

import { document } from "incajs";

const a = ref(null);
const b = ref(null);
const c = ref(null);
const log = ref("");
const active = ref("");

function note(token) {
  log.value = log.value ? `${log.value} ${token}` : token;
}

function onFocusA() {
  b.value.focus();
}

function onFocusC() {
  c.value.blur();
}

function onKeydownB(e) {
  note(`keydown:${e.currentTarget.id}`);
}

function goA() {
  a.value.focus();
}

function goC() {
  c.value.focus();
}

function read() {
  active.value = document.activeElement.id;
}
</script>

<template>
  <div :style="{ width: 300, height: 800 }">
    <div id="a" ref="a" tabindex="0" :style="{ width: 100, height: 40 }" @focus="onFocusA" />
    <div id="b" ref="b" tabindex="0" :style="{ width: 100, height: 40 }" @keydown="onKeydownB" />
    <div id="c" ref="c" tabindex="0" :style="{ width: 100, height: 40 }" @focus="onFocusC" />
    <div id="go-a" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="goA" />
    <div id="go-c" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="goC" />
    <div id="read" :style="{ width: 100, height: 40 }" @mousedown.prevent @click="read" />
    <div id="log">{{ log }}</div>
    <div id="active">{{ active }}</div>
  </div>
</template>
