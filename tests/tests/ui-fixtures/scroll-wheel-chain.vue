<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const text = ref("");
const inner1 = ref(null);
const outer5 = ref(null);

function log(e) {
  text.value += `wheel@${e.currentTarget.id}:${e.deltaX}:${e.deltaY}:${e.deltaMode} `;
}

function cancel(e) {
  log(e);
  e.preventDefault();
}

function stop(e) {
  log(e);
  e.stopPropagation();
}

function passive(e) {
  log(e);
  e.preventDefault();
  text.value += `passive:${e.defaultPrevented} `;
}

const onInner = { 3: stop };
const onOuter = { 2: cancel };
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'row' }">
    <div id="outer-1" :style="{ width: 100, height: 100, overflow: 'auto' }" @wheel="log">
      <div
        id="inner-1"
        ref="inner1"
        :style="{ width: 100, height: 60, overflow: 'auto' }"
        @wheel="log"
      >
        <div id="i0-1" :style="{ width: 100, height: 40 }" />
        <div id="i1-1" :style="{ width: 100, height: 40 }" />
        <div id="i2-1" :style="{ width: 100, height: 40 }" />
      </div>
      <div id="tail-1" :style="{ width: 100, height: 100 }" />
    </div>
    <div
      v-for="n in [2, 3]"
      :id="`outer-${n}`"
      :key="n"
      :style="{ width: 100, height: 100, overflow: 'auto' }"
      @wheel="(onOuter[n] ?? log)($event)"
    >
      <div
        :id="`inner-${n}`"
        :style="{ width: 100, height: 60, overflow: 'auto' }"
        @wheel="(onInner[n] ?? log)($event)"
      >
        <div :id="`i0-${n}`" :style="{ width: 100, height: 40 }" />
        <div :id="`i1-${n}`" :style="{ width: 100, height: 40 }" />
        <div :id="`i2-${n}`" :style="{ width: 100, height: 40 }" />
      </div>
      <div :id="`tail-${n}`" :style="{ width: 100, height: 100 }" />
    </div>
    <div id="outer-4" :style="{ width: 100, height: 100, overflow: 'auto' }" @wheel="log">
      <div
        id="inner-4"
        :style="{ width: 100, height: 60, overflow: 'auto' }"
        @wheel.passive="passive"
      >
        <div id="i0-4" :style="{ width: 100, height: 40 }" />
        <div id="i1-4" :style="{ width: 100, height: 40 }" />
        <div id="i2-4" :style="{ width: 100, height: 40 }" />
      </div>
      <div id="tail-4" :style="{ width: 100, height: 100 }" />
    </div>
    <div
      id="outer-5"
      ref="outer5"
      :style="{ width: 100, height: 100, overflow: 'auto' }"
      @wheel="log"
    >
      <div id="head-5" :style="{ width: 100, height: 100 }" />
      <div id="inner-5" :style="{ width: 100, height: 60, overflow: 'auto' }" @wheel="log">
        <div id="i0-5" :style="{ width: 100, height: 40 }" />
        <div id="i1-5" :style="{ width: 100, height: 40 }" />
        <div id="i2-5" :style="{ width: 100, height: 40 }" />
      </div>
    </div>
  </div>
  <div :style="{ display: 'flex', flex_direction: 'row' }">
    <div
      id="end-1"
      :style="{ width: 12, height: 12 }"
      @mousedown.prevent
      @click="inner1.scrollTop = 60"
    />
    <div
      id="end-5"
      :style="{ width: 12, height: 12 }"
      @mousedown.prevent
      @click="outer5.scrollTop = 60"
    />
  </div>
  <div id="log">{{ text }}</div>
</template>
