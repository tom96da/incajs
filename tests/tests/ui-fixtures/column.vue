<!--
Copyright (c) 2026 tom96da
SPDX-License-Identifier: MIT OR Apache-2.0
-->

<template>
  <div
    id="column"
    :style="{
      display: 'flex',
      flex_direction: 'column',
      width: 100,
      height: 310,
      padding: 10,
      gap: 8,
    }"
  >
    <div id="a" :style="{ width: 50, height: 40 }" />
    <div id="b" :style="{ width: 50, flex_grow: 1 }" />
    <div id="c" :style="{ width: 50, flex_grow: 2 }" />
  </div>
</template>
