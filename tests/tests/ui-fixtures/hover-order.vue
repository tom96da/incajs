<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const text = ref("");
const at = ref("");

function log(e) {
  const related = e.relatedTarget ? e.relatedTarget.id : "null";
  text.value += `${e.type}@${e.target.id}<${related} `;
}

function pos(e) {
  at.value = `${e.type}:${e.clientX}:${e.clientY}`;
}
</script>

<template>
  <div
    id="a"
    :style="{ width: 100, height: 40 }"
    @pointerout="log"
    @pointerleave="log"
    @pointerover="log"
    @pointerenter="log"
    @mouseout="log"
    @mouseleave="log"
    @mouseover="log"
    @mouseenter="log"
    @pointermove="log"
    @mousemove="(log($event), pos($event))"
  />
  <div
    id="b"
    :style="{ width: 100, height: 40 }"
    @pointerout="log"
    @pointerleave="log"
    @pointerover="log"
    @pointerenter="log"
    @mouseout="log"
    @mouseleave="log"
    @mouseover="log"
    @mouseenter="log"
    @pointermove="log"
    @mousemove="log"
  />
  <div id="log">{{ text }}</div>
  <div id="pos">{{ at }}</div>
</template>
