// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

// The host defines these globals before the app runs. This file carries their types.
declare global {
  /** The document of the app. The build adds its import to every module that names it. */
  var document: import("incajs").IncaDocument;

  /** The interface object of `document`. */
  var Document: { readonly prototype: object };

  /** The interface object of shadow roots, which the host never creates. */
  var ShadowRoot: { readonly prototype: object };

  /** The live set of the tokens of an attribute. It is read from `classList`. */
  interface DOMTokenList {
    /** The number of tokens. */
    readonly length: number;
    /** The attribute text. Assigning sets the attribute. */
    value: string;
    toString(): string;
    /** Returns the token at `index`, or `null`. */
    item(index: number): string | null;
    contains(token: string): boolean;
    add(...tokens: string[]): void;
    remove(...tokens: string[]): void;
    toggle(token: string, force?: boolean): boolean;
    replace(token: string, newToken: string): boolean;
    /** Throws a `TypeError`, because `class` defines no supported tokens. */
    supports(token: string): boolean;
    forEach(
      callback: (token: string, index: number, list: DOMTokenList) => void,
      thisArg?: unknown,
    ): void;
    keys(): IteratorObject<number, undefined, unknown>;
    values(): IteratorObject<string, undefined, unknown>;
    entries(): IteratorObject<[number, string], undefined, unknown>;
    [Symbol.iterator](): IteratorObject<string, undefined, unknown>;
  }

  /** The interface object of token lists. It cannot be called or constructed. */
  var DOMTokenList: { readonly prototype: DOMTokenList };

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

  interface PerformanceEntry {
    readonly name: string;
    readonly entryType: string;
    readonly startTime: number;
    readonly duration: number;
    toJSON(): any;
  }
  var PerformanceEntry: { prototype: PerformanceEntry; new (): PerformanceEntry };
  type PerformanceEntryList = PerformanceEntry[];

  interface PerformanceMarkOptions {
    detail?: any;
    startTime?: number;
  }
  interface PerformanceMark extends PerformanceEntry {
    readonly detail: any;
  }
  var PerformanceMark: {
    prototype: PerformanceMark;
    new (markName: string, markOptions?: PerformanceMarkOptions): PerformanceMark;
  };

  interface PerformanceMeasureOptions {
    detail?: any;
    start?: string | number;
    duration?: number;
    end?: string | number;
  }
  interface PerformanceMeasure extends PerformanceEntry {
    readonly detail: any;
  }
  var PerformanceMeasure: { prototype: PerformanceMeasure; new (): PerformanceMeasure };

  interface Performance {
    readonly timeOrigin: number;
    now(): number;
    toJSON(): any;
    mark(markName: string, markOptions?: PerformanceMarkOptions): PerformanceMark;
    measure(
      measureName: string,
      startOrMeasureOptions?: string | PerformanceMeasureOptions,
      endMark?: string,
    ): PerformanceMeasure;
    getEntries(): PerformanceEntryList;
    getEntriesByName(name: string, type?: string): PerformanceEntryList;
    getEntriesByType(type: string): PerformanceEntryList;
    clearMarks(markName?: string): void;
    clearMeasures(measureName?: string): void;
  }
  var Performance: { prototype: Performance; new (): Performance };
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

// The JavaScript engine provides these globals.
declare global {
  interface DOMException {
    readonly name: string;
    readonly message: string;
    readonly code: number;
    stack?: string;
    toString(): string;
    readonly INDEX_SIZE_ERR: 1;
    readonly DOMSTRING_SIZE_ERR: 2;
    readonly HIERARCHY_REQUEST_ERR: 3;
    readonly WRONG_DOCUMENT_ERR: 4;
    readonly INVALID_CHARACTER_ERR: 5;
    readonly NO_DATA_ALLOWED_ERR: 6;
    readonly NO_MODIFICATION_ALLOWED_ERR: 7;
    readonly NOT_FOUND_ERR: 8;
    readonly NOT_SUPPORTED_ERR: 9;
    readonly INUSE_ATTRIBUTE_ERR: 10;
    readonly INVALID_STATE_ERR: 11;
    readonly SYNTAX_ERR: 12;
    readonly INVALID_MODIFICATION_ERR: 13;
    readonly NAMESPACE_ERR: 14;
    readonly INVALID_ACCESS_ERR: 15;
    readonly VALIDATION_ERR: 16;
    readonly TYPE_MISMATCH_ERR: 17;
    readonly SECURITY_ERR: 18;
    readonly NETWORK_ERR: 19;
    readonly ABORT_ERR: 20;
    readonly URL_MISMATCH_ERR: 21;
    readonly QUOTA_EXCEEDED_ERR: 22;
    readonly TIMEOUT_ERR: 23;
    readonly INVALID_NODE_TYPE_ERR: 24;
    readonly DATA_CLONE_ERR: 25;
  }
  var DOMException: {
    prototype: DOMException;
    new (message?: string, name?: string): DOMException;
    readonly INDEX_SIZE_ERR: 1;
    readonly DOMSTRING_SIZE_ERR: 2;
    readonly HIERARCHY_REQUEST_ERR: 3;
    readonly WRONG_DOCUMENT_ERR: 4;
    readonly INVALID_CHARACTER_ERR: 5;
    readonly NO_DATA_ALLOWED_ERR: 6;
    readonly NO_MODIFICATION_ALLOWED_ERR: 7;
    readonly NOT_FOUND_ERR: 8;
    readonly NOT_SUPPORTED_ERR: 9;
    readonly INUSE_ATTRIBUTE_ERR: 10;
    readonly INVALID_STATE_ERR: 11;
    readonly SYNTAX_ERR: 12;
    readonly INVALID_MODIFICATION_ERR: 13;
    readonly NAMESPACE_ERR: 14;
    readonly INVALID_ACCESS_ERR: 15;
    readonly VALIDATION_ERR: 16;
    readonly TYPE_MISMATCH_ERR: 17;
    readonly SECURITY_ERR: 18;
    readonly NETWORK_ERR: 19;
    readonly ABORT_ERR: 20;
    readonly URL_MISMATCH_ERR: 21;
    readonly QUOTA_EXCEEDED_ERR: 22;
    readonly TIMEOUT_ERR: 23;
    readonly INVALID_NODE_TYPE_ERR: 24;
    readonly DATA_CLONE_ERR: 25;
  };
}

export {};
