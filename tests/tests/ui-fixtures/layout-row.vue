<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<template>
  <div
    id="row"
    :style="{
      display: 'flex',
      flex_direction: 'row',
      width: 400,
      height: 100,
      padding: 20,
      gap: 10,
    }"
  >
    <div id="a" :style="{ width: 50, height: 30 }" />
    <div id="b" :style="{ width: 60, height: 30 }" />
    <div id="c" :style="{ flex_grow: 1, height: 30 }" />
  </div>
</template>
