<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { onMounted, ref } from "@vue/runtime-core";

import { document, window } from "incajs";

const log = ref("");
const since = ref("");
const related = ref("");
const active = ref("");
const value = ref("abcd");
const buttons = ref("");
const mm = ref(0);
const keys = ref("");
const present = ref(true);
const disabled = ref(false);
const field = ref(null);
const css =
  ".f { width: 100px } .f:focus { width: 120px } #btn { width: 80px } #btn:active { width: 90px }";
let armed = "";

const note = (type, id) => {
  log.value += `${type}@${id} `;
  since.value += `${type}@${id} `;
};
const noteRelated = (e) => {
  related.value += `${e.type}:${e.relatedTarget ? e.relatedTarget.id : "null"} `;
};
const actions = {
  F2: () => field.value.focus(),
  F4: () => {
    field.value.focus();
    field.value.blur();
  },
  F6: () => (disabled.value = true),
  F7: () => (present.value = false),
};

onMounted(() => {
  window.addEventListener("focus", () => note("focus", "window"));
  window.addEventListener("blur", () => {
    note("blur", "window");
    active.value = document.activeElement.id;
  });
  window.addEventListener("blur", () => {
    since.value = "";
    const action = actions[armed];
    armed = "";
    if (action) action();
  });
  window.addEventListener("mousemove", (e) => {
    buttons.value = String(e.buttons);
    mm.value += 1;
  });
  window.addEventListener("keydown", (e) => {
    if (e.key === "Shift") keys.value += "Shift ";
    if (e.key in actions) armed = e.key;
    if (e.key === "F9") {
      log.value = since.value = related.value = active.value = buttons.value = keys.value = "";
      mm.value = 0;
    }
  });
});
</script>

<template>
  <div :style="{ width: 300, height: 400, display: 'flex', flex_direction: 'column' }">
    <component is="style">{{ css }}</component>
    <input
      v-if="present"
      id="field"
      ref="field"
      v-model="value"
      class="f"
      :disabled="disabled"
      @focus="note('focus', 'field')"
      @blur="note('blur', 'field')"
      @focusin="
        note('focusin', 'field');
        noteRelated($event);
      "
      @focusout="
        note('focusout', 'field');
        noteRelated($event);
      "
      @change="note('change', 'field')"
    />
    <button id="btn" :style="{ height: 30 }" />
    <div id="pad" :style="{ width: 80, height: 30 }" @pointerdown="$event.preventDefault()" />
    <div id="log">{{ log }}</div>
    <div id="since">{{ since }}</div>
    <div id="related">{{ related }}</div>
    <div id="active">{{ active }}</div>
    <div id="value">{{ value }}</div>
    <div id="buttons">{{ buttons }}</div>
    <div id="mm">{{ mm }}</div>
    <div id="keys">{{ keys }}</div>
  </div>
</template>
