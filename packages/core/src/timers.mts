// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The host defines these functions before the app runs. This file carries their types.

declare global {
  function setTimeout<A extends unknown[]>(
    handler: (...args: A) => void,
    delay?: number,
    ...args: A
  ): number;
  function setInterval<A extends unknown[]>(
    handler: (...args: A) => void,
    delay?: number,
    ...args: A
  ): number;
  function clearTimeout(id?: number): void;
  function clearInterval(id?: number): void;
  function requestAnimationFrame(callback: (time: number) => void): number;
  function cancelAnimationFrame(id: number): void;
  interface Performance {
    now(): number;
  }
  var performance: Performance;
}

export {};
