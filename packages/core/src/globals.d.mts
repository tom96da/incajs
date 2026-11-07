// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The host defines these globals before the app runs. This file carries their types.

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

  type FrameRequestCallback = (time: number) => void;
  function requestAnimationFrame(callback: FrameRequestCallback): number;
  function cancelAnimationFrame(handle: number): void;

  interface Performance {
    now(): number;
  }
  var performance: Performance;

  interface Console {
    assert(condition?: boolean, ...data: unknown[]): void;
    clear(): void;
    count(label?: string): void;
    countReset(label?: string): void;
    debug(...data: unknown[]): void;
    dir(item?: unknown, options?: unknown): void;
    dirxml(...data: unknown[]): void;
    error(...data: unknown[]): void;
    group(...data: unknown[]): void;
    groupCollapsed(...data: unknown[]): void;
    groupEnd(): void;
    info(...data: unknown[]): void;
    log(...data: unknown[]): void;
    table(tabularData?: unknown, properties?: string[]): void;
    time(label?: string): void;
    timeEnd(label?: string): void;
    timeLog(label?: string, ...data: unknown[]): void;
    trace(...data: unknown[]): void;
    warn(...data: unknown[]): void;
  }
  var console: Console;

  interface TextEncoderEncodeIntoResult {
    read: number;
    written: number;
  }
  interface TextEncoder {
    readonly encoding: string;
    encode(input?: string): Uint8Array<ArrayBuffer>;
    encodeInto(source: string, destination: Uint8Array): TextEncoderEncodeIntoResult;
  }
  var TextEncoder: {
    new (): TextEncoder;
    prototype: TextEncoder;
  };

  type AllowSharedBufferSource = ArrayBufferLike | ArrayBufferView;
  interface TextDecoderOptions {
    fatal?: boolean;
    ignoreBOM?: boolean;
  }
  interface TextDecodeOptions {
    stream?: boolean;
  }
  interface TextDecoder {
    readonly encoding: string;
    readonly fatal: boolean;
    readonly ignoreBOM: boolean;
    decode(input?: AllowSharedBufferSource, options?: TextDecodeOptions): string;
  }
  var TextDecoder: {
    new (label?: string, options?: TextDecoderOptions): TextDecoder;
    prototype: TextDecoder;
  };

  interface URL {
    href: string;
    protocol: string;
    username: string;
    password: string;
    host: string;
    hostname: string;
    port: string;
    pathname: string;
    search: string;
    hash: string;
    readonly origin: string;
    readonly searchParams: URLSearchParams;
    toString(): string;
    toJSON(): string;
  }
  var URL: {
    new (url: string | URL, base?: string | URL): URL;
    prototype: URL;
    canParse(url: string | URL, base?: string | URL): boolean;
    parse(url: string | URL, base?: string | URL): URL | null;
  };

  interface URLSearchParamsIterator<T> extends IteratorObject<T, BuiltinIteratorReturn, unknown> {
    [Symbol.iterator](): URLSearchParamsIterator<T>;
  }
  interface URLSearchParams {
    readonly size: number;
    append(name: string, value: string): void;
    delete(name: string, value?: string): void;
    get(name: string): string | null;
    getAll(name: string): string[];
    has(name: string, value?: string): boolean;
    set(name: string, value: string): void;
    sort(): void;
    forEach(
      callback: (value: string, key: string, parent: URLSearchParams) => void,
      thisArg?: unknown,
    ): void;
    entries(): URLSearchParamsIterator<[string, string]>;
    keys(): URLSearchParamsIterator<string>;
    values(): URLSearchParamsIterator<string>;
    [Symbol.iterator](): URLSearchParamsIterator<[string, string]>;
    toString(): string;
  }
  var URLSearchParams: {
    new (init?: string | string[][] | Record<string, string> | URLSearchParams): URLSearchParams;
    prototype: URLSearchParams;
  };
}

export {};
