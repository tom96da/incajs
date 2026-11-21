<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const mouse = ref("");
const pointer = ref("");
const key = ref("");
const focus = ref("");
const wheel = ref("");
const scroll = ref("");
const input = ref("");
const prevented = ref("");
const cls = ref("");

const CLASSED = ["click", "dblclick", "contextmenu", "auxclick", "mousedown", "scroll"];
const PREVENTED = ["mouseenter", "mouseleave", "scroll", "focus", "focusin", "input", "change"];

const flags = (e) => `${e.type}:${e.bubbles}:${e.cancelable}:${e.composed} `;

function note(e) {
  if (CLASSED.includes(e.type)) {
    cls.value += `${e.type}:${Object.prototype.toString.call(e).slice(8, -1)} `;
  }
  if (PREVENTED.includes(e.type)) {
    e.preventDefault();
    prevented.value += `${e.type}:${e.defaultPrevented} `;
  }
}

const onMouse = (e) => {
  mouse.value += flags(e);
  note(e);
};
const onPointer = (e) => (pointer.value += flags(e));
const onKey = (e) => (key.value += flags(e));
const onFocus = (e) => {
  focus.value += flags(e);
  note(e);
};
const onWheel = (e) => (wheel.value += flags(e));
const onScroll = (e) => {
  scroll.value += flags(e);
  note(e);
};
const onInput = (e) => {
  input.value += flags(e);
  note(e);
};
</script>

<template>
  <div :style="{ width: 300, height: 400 }">
    <div
      v-for="id in ['plain', 'plain2']"
      :id="id"
      :key="id"
      :style="{ width: 100, height: 40 }"
      @mousedown="onMouse"
      @mouseup="onMouse"
      @mousemove="onMouse"
      @mouseenter="onMouse"
      @mouseleave="onMouse"
      @mouseover="onMouse"
      @mouseout="onMouse"
      @click="onMouse"
      @dblclick="onMouse"
      @contextmenu="onMouse"
      @auxclick="onMouse"
      @pointerdown="onPointer"
      @pointerup="onPointer"
      @pointermove="onPointer"
      @pointerenter="onPointer"
      @pointerleave="onPointer"
      @pointerover="onPointer"
      @pointerout="onPointer"
    />
    <div
      id="box"
      tabindex="0"
      :style="{ width: 100, height: 40 }"
      @keydown="onKey"
      @keypress="onKey"
      @keyup="onKey"
      @focus="onFocus"
      @blur="onFocus"
      @focusin="onFocus"
      @focusout="onFocus"
    />
    <div
      id="scroller"
      :style="{ width: 100, height: 60, overflow: 'auto' }"
      @wheel="onWheel"
      @scroll="onScroll"
    >
      <div :style="{ width: 80, height: 120 }" />
    </div>
    <input id="field" @beforeinput="onInput" @input="onInput" @change="onInput" />
    <div id="mouse">{{ mouse }}</div>
    <div id="pointer">{{ pointer }}</div>
    <div id="key">{{ key }}</div>
    <div id="focus">{{ focus }}</div>
    <div id="wheel">{{ wheel }}</div>
    <div id="scroll">{{ scroll }}</div>
    <div id="input">{{ input }}</div>
    <div id="prevented">{{ prevented }}</div>
    <div id="class">{{ cls }}</div>
  </div>
</template>
