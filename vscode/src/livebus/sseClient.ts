import * as vscode from "vscode";
import * as http from "node:http";
import { getConfig, logAt } from "../util/config";

/**
 * Minimal SSE client for `mneme-livebus`.
 *
 * We can't rely on `eventsource` or `node-fetch` being installed (the
 * extension carries only @types/node). Node's built-in `http` is enough:
 * we make a long-lived GET to `/events` and parse `event:`/`data:` lines as
 * they arrive.
 *
 * Reconnect strategy: exponential backoff capped at 30s. Reconnects are
 * scheduled as timers, not recursion, so the extension host can clean up
 * cleanly on dispose.
 */

export type LiveEvent =
  | { readonly type: "job.complete"; readonly data: unknown }
  | { readonly type: "drift.finding"; readonly data: unknown }
  | { readonly type: "step.complete"; readonly data: unknown }
  | { readonly type: "graph.updated"; readonly data: unknown }
  | { readonly type: "connected"; readonly data: null }
  | { readonly type: "disconnected"; readonly data: { readonly reason: string } }
  | { readonly type: "other"; readonly data: unknown; readonly rawName: string };

export type LiveListener = (event: LiveEvent) => void;

export interface SseClientOptions {
  readonly host?: string;
  readonly path?: string;
}

export class LiveBusClient implements vscode.Disposable {
  private request: http.ClientRequest | null = null;
  private response: http.IncomingMessage | null = null;
  private reconnectTimer: NodeJS.Timeout | null = null;
  private backoffMs = 1_000;
  private disposed = false;
  private buffer = "";
  private currentEventName = "message";
  private currentEventData: string[] = [];

  private readonly listeners = new Set<LiveListener>();
  private readonly channel: vscode.OutputChannel | null;
  private readonly host: string;
  private readonly path: string;

  public constructor(
    channel: vscode.OutputChannel | null,
    options: SseClientOptions = {},
  ) {
    this.channel = channel;
    this.host = options.host ?? "127.0.0.1";
    this.path = options.path ?? "/events";
  }

  public onEvent(listener: LiveListener): vscode.Disposable {
    this.listeners.add(listener);
    return new vscode.Disposable(() => {
      this.listeners.delete(listener);
    });
  }

  public start(): void {
    if (this.disposed) {
      return;
    }
    if (this.request !== null) {
      return;
    }
    const port = getConfig().graphViewPort;
    logAt(this.channel, "debug", `livebus: connecting to http://${this.host}:${port}${this.path}`);

    const req = http.request(
      {
        host: this.host,
        port,
        path: this.path,
        method: "GET",
        headers: {
          accept: "text/event-stream",
          "cache-control": "no-cache",
        },
      },
      (res) => this.onResponse(res),
    );

    req.on("error", (err) => this.onError(err));
    req.on("close", () => this.onClose("request closed"));
    req.setTimeout(0);
    req.end();
    this.request = req;
  }

  public stop(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.response) {
      try {
        this.response.destroy();
      } catch {
        // Ignore.
      }
      this.response = null;
    }
    if (this.request) {
      try {
        this.request.destroy();
      } catch {
        // Ignore.
      }
      this.request = null;
    }
    this.buffer = "";
    this.currentEventData = [];
    this.currentEventName = "message";
  }

  public dispose(): void {
    this.disposed = true;
    this.stop();
    this.listeners.clear();
  }

  private onResponse(res: http.IncomingMessage): void {
    if (this.disposed) {
      try {
        res.destroy();
      } catch {
        // Ignore.
      }
      return;
    }
    if (res.statusCode !== 200) {
      this.onClose(`http ${res.statusCode ?? "unknown"}`);
      return;
    }
    this.response = res;
    this.backoffMs = 1_000;
    res.setEncoding("utf8");
    this.emit({ type: "connected", data: null });

    res.on("data", (chunk: string) => this.onChunk(chunk));
    res.on("end", () => this.onClose("stream ended"));
    res.on("error", (err) => this.onError(err));
  }

  private onChunk(chunk: string): void {
    this.buffer += chunk;
    let idx: number;
    while ((idx = this.buffer.indexOf("\n")) >= 0) {
      const rawLine = this.buffer.slice(0, idx);
      this.buffer = this.buffer.slice(idx + 1);
      const line = rawLine.replace(/\r$/, "");
      this.onLine(line);
    }
  }

  private onLine(line: string): void {
    if (line.length === 0) {
      // Dispatch pending event on blank line.
      if (this.currentEventData.length > 0) {
        const data = this.currentEventData.join("\n");
        const parsed = safeParseJson(data);
        this.dispatchByName(this.currentEventName, parsed);
      }
      this.currentEventName = "message";
      this.currentEventData = [];
      return;
    }
    if (line.startsWith(":")) {
      // SSE comment / keepalive.
      return;
    }
    const colonIdx = line.indexOf(":");
    const field = colonIdx >= 0 ? line.slice(0, colonIdx) : line;
    const value =
      colonIdx >= 0 ? line.slice(colonIdx + 1).replace(/^\s/, "") : "";
    switch (field) {
      case "event":
        this.currentEventName = value;
        return;
      case "data":
        this.currentEventData.push(value);
        return;
      default:
        return;
    }
  }

  private dispatchByName(name: string, data: unknown): void {
    switch (name) {
      case "job.complete":
        this.emit({ type: "job.complete", data });
        return;
      case "drift.finding":
        this.emit({ type: "drift.finding", data });
        return;
      case "step.complete":
        this.emit({ type: "step.complete", data });
        return;
      case "graph.updated":
        this.emit({ type: "graph.updated", data });
        return;
      default:
        this.emit({ type: "other", rawName: name, data });
        return;
    }
  }

  private emit(event: LiveEvent): void {
    for (const listener of this.listeners) {
      try {
        listener(event);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        logAt(this.channel, "warn", `livebus listener threw: ${message}`);
      }
    }
  }

  private onError(err: Error): void {
    logAt(this.channel, "debug", `livebus error: ${err.message}`);
    this.onClose(err.message);
  }

  private onClose(reason: string): void {
    if (this.disposed) {
      return;
    }
    this.emit({ type: "disconnected", data: { reason } });
    this.stop();
    this.scheduleReconnect();
  }

  private scheduleReconnect(): void {
    if (this.disposed) {
      return;
    }
    const delay = this.backoffMs;
    this.backoffMs = Math.min(30_000, Math.floor(this.backoffMs * 2));
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.start();
    }, delay);
  }
}

function safeParseJson(input: string): unknown {
  if (input.length === 0) {
    return null;
  }
  try {
    return JSON.parse(input);
  } catch {
    return input;
  }
}
