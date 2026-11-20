<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<script setup>
import { ref } from "@vue/runtime-core";

const items = ref([1, 2, 3]);
let next = 4;
const add = () => items.value.push(next++);
const remove = () => items.value.shift();
const reverse = () => items.value.reverse();
const swap = () => {
  const list = items.value;
  [list[0], list[list.length - 1]] = [list[list.length - 1], list[0]];
};
const insert = () => items.value.splice(1, 0, next++);
</script>

<template>
  <div :style="{ display: 'flex', flex_direction: 'column' }">
    <div id="add" :style="{ width: 100, height: 30 }" @click="add">Add</div>
    <div id="remove" :style="{ width: 100, height: 30 }" @click="remove">Remove</div>
    <div id="reverse" :style="{ width: 100, height: 30 }" @click="reverse">Reverse</div>
    <div id="swap" :style="{ width: 100, height: 30 }" @click="swap">Swap</div>
    <div id="insert" :style="{ width: 100, height: 30 }" @click="insert">Insert</div>
    <div id="list" :style="{ display: 'flex', flex_direction: 'column' }">
      <div
        v-for="item in items"
        :id="`item-${item}`"
        :key="item"
        :style="{ width: 100, height: 20 }"
      >
        Item {{ item }}
      </div>
    </div>
  </div>
</template>
