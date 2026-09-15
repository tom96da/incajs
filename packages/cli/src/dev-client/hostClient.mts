// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

import { spawn } from "node:child_process";
import { once } from "node:events";
import readline from "node:readline";
import { setTimeout as delay } from "node:timers/promises";
import type { ChildProcessWithoutNullStreams } from "node:child_process";

import { resolveHostBin } from "./hostBin.mts";
import { HostError } from "./hostError.mts";
import { HOST_PROTOCOL_VERSION, JSONRPC, isRpcMessage } from "./protocol.mts";
import type { AppErrorParams, ReadyParams } from "./protocol.mts";

interface PendingCall {
  resolve: (result: unknown) => void;
  reject: (error: Error) => void;
}

export interface HostClientOptions {
  /** The bundle passed to the host as `--dev <bundlePath>`. */
  bundlePath: string;
  /** Overrides which `gpjs-ui-host` binary gets spawned, in place of automatic resolution. */
  hostBin?: string;
  /**
   * Every diagnostic line the transport itself produces: the host's real
   * stderr, a stray stdout line, and a `ready` protocol mismatch. Defaults
   * to this process's own stderr.
   */
  onStderr?: (line: string) => void;
  /**
   * The window is up and the first bundle has been evaluated, and the host
   * speaks the protocol revision this package was built for.
   */
  onReady?: () => void;
  /** An app's own event listener threw; the host caught it and kept rendering. */
  onAppError?: (error: AppErrorParams) => void;
  /**
   * Handlers for notification `method`s this package doesn't implement
   * itself (e.g. a future Vite integration), keyed by method name —
   * `params` forwarded untouched.
   */
  integrations?: Record<string, (params: unknown) => void>;
}

const defaultOnStderr = (line: string): void => {
  process.stderr.write(line.endsWith("\n") ? line : `${line}\n`);
};

/**
 * Spawns `gpjs-ui-host --dev <bundlePath>` and speaks newline-delimited
 * JSON-RPC 2.0 on its stdin/stdout. One instance owns exactly one child
 * process for its whole lifetime — a reload is a `reload` call on the same
 * child, never a respawn.
 */
export class HostClient {
  #options: HostClientOptions;
  #child: ChildProcessWithoutNullStreams | undefined;
  #nextId = 1;
  #pending = new Map<number, PendingCall>();

  constructor(options: HostClientOptions) {
    this.#options = options;
  }

  /**
   * Starts the child and wires up its streams. Resolves once the process
   * has actually spawned; rejects if it never does (e.g. the resolved
   * binary doesn't exist).
   */
  async start(): Promise<void> {
    const hostBin = this.#options.hostBin ?? resolveHostBin();
    const child = spawn(hostBin, ["--dev", this.#options.bundlePath], {
      stdio: ["pipe", "pipe", "pipe"],
    });
    this.#child = child;

    const onStderr = this.#options.onStderr ?? defaultOnStderr;
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk: string) => onStderr(chunk));

    readline.createInterface({ input: child.stdout }).on("line", (line) => {
      this.#handleLine(line, onStderr);
    });

    child.on("exit", (code, signal) => {
      const reason = new Error(
        `gpjs-ui-host exited (code=${String(code)}, signal=${String(signal)}) before answering`,
      );
      for (const { reject } of this.#pending.values()) reject(reason);
      this.#pending.clear();
    });

    await once(child, "spawn");
  }

  #handleLine(line: string, onStderr: (line: string) => void): void {
    let message: unknown;
    try {
      message = JSON.parse(line);
    } catch {
      onStderr(`[stray stdout] ${line}`);
      return;
    }
    if (!isRpcMessage(message)) {
      onStderr(`[stray stdout] ${line}`);
      return;
    }

    if ("method" in message) {
      this.#handleNotification(message.method, message.params, onStderr);
      return;
    }

    const pending = this.#pending.get(message.id);
    if (!pending) return; // a response to a call nobody is waiting for anymore
    this.#pending.delete(message.id);

    if ("error" in message) {
      pending.reject(
        new HostError(message.error.message, message.error.code, message.error.data?.stack ?? null),
      );
    } else {
      pending.resolve(message.result);
    }
  }

  #handleNotification(method: string, params: unknown, onStderr: (line: string) => void): void {
    switch (method) {
      case "ready": {
        const { protocol } = params as ReadyParams;
        if (protocol !== HOST_PROTOCOL_VERSION) {
          onStderr(
            `gpjs-ui-host speaks protocol ${String(protocol)}, this package was built for ` +
              `${String(HOST_PROTOCOL_VERSION)} — stopping it rather than carrying on`,
          );
          if (this.#child) void this.#kill(this.#child);
          return;
        }
        this.#options.onReady?.();
        return;
      }
      case "appError":
        this.#options.onAppError?.(params as AppErrorParams);
        return;
      default: {
        const integration = this.#options.integrations?.[method];
        if (integration) integration(params);
        else onStderr(`[unhandled notification] ${method}`);
      }
    }
  }

  /** Kills the child and waits for it to actually exit. */
  async #kill(child: ChildProcessWithoutNullStreams): Promise<void> {
    const exited = once(child, "exit");
    child.kill();
    await exited;
  }

  /** Sends a request and resolves with its result once the host answers. */
  async call(method: string, params?: unknown): Promise<unknown> {
    const child = this.#child;
    if (!child) throw new Error("HostClient.start() has not been called");

    const id = this.#nextId++;
    const line = JSON.stringify({ jsonrpc: JSONRPC, id, method, params });
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      child.stdin.write(`${line}\n`);
    });
  }

  /**
   * Asks the host to `shutdown` and waits for the child to actually exit —
   * that exit is the real acknowledgement, not the response. Kills it once
   * `timeoutMs` passes without that, so a wedged app can't block teardown.
   */
  async stop(timeoutMs = 2000): Promise<void> {
    const child = this.#child;
    if (!child || child.exitCode !== null || child.signalCode !== null) return;

    const exited = once(child, "exit");
    this.call("shutdown").catch(() => {
      // The pipe can close before a response arrives; the exit below is
      // what actually matters.
    });

    const outcome = await Promise.race([
      exited.then(() => "exited" as const),
      delay(timeoutMs).then(() => "timeout" as const),
    ]);
    if (outcome === "timeout") {
      child.kill();
      await exited;
    }
  }
}
