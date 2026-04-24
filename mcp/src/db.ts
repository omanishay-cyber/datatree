/**
 * mneme CLI IPC wrapper.
 *
 * Exposes the same 7 sub-layer DB API (Builder / Finder / AccessPath / Query
 * / Response / Injection / Lifecycle — see design §13.5) to TypeScript callers.
 *
 * The MCP server NEVER opens SQLite directly — every read or write goes
 * through the Rust supervisor over a length-prefixed JSON IPC framing on a
 * Unix-domain socket (POSIX) or named pipe (Windows). This keeps the
 * single-writer-per-shard invariant from §3.4 in force.
 */

import { createConnection, type Socket } from "node:net";
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { homedir, platform } from "node:os";
import { join } from "node:path";
import type {
  DbLayer,
  Decision,
  Finding,
  IpcRequest,
  IpcResponse,
  Step,
} from "./types.ts";

// ---------------------------------------------------------------------------
// Socket discovery
// ---------------------------------------------------------------------------

/**
 * Discover the supervisor IPC endpoint:
 *   - Windows: \\?\pipe\mneme-supervisor
 *   - macOS / Linux: $HOME/.mneme/supervisor.sock
 *
 * Override via MNEME_SOCKET env var.
 */
function discoverSocketPath(): string {
  const override = process.env.MNEME_SOCKET;
  if (override && override.length > 0) {
    return override;
  }
  if (platform() === "win32") {
    return "\\\\?\\pipe\\mneme-supervisor";
  }
  return join(homedir(), ".mneme", "supervisor.sock");
}

// ---------------------------------------------------------------------------
// IPC client (length-prefixed JSON framing)
// ---------------------------------------------------------------------------

interface PendingRequest {
  resolve: (response: IpcResponse) => void;
  reject: (err: Error) => void;
  startedAt: number;
  timeoutHandle: ReturnType<typeof setTimeout>;
}

class IpcClient {
  private socket: Socket | null = null;
  private connectPromise: Promise<void> | null = null;
  private buffer: Buffer = Buffer.alloc(0);
  private pending = new Map<string, PendingRequest>();
  private reconnectAttempts = 0;
  private readonly MAX_RECONNECT = 5;
  private readonly REQUEST_TIMEOUT_MS = 30_000;

  constructor(private readonly socketPath: string) {}

  private async connect(): Promise<void> {
    if (this.socket && !this.socket.destroyed) return;
    if (this.connectPromise) return this.connectPromise;

    this.connectPromise = new Promise<void>((resolve, reject) => {
      const sock = createConnection(this.socketPath, () => {
        this.reconnectAttempts = 0;
        this.socket = sock;
        this.connectPromise = null;
        resolve();
      });
      sock.setNoDelay(true);
      sock.on("data", (chunk) => this.onData(chunk));
      sock.on("error", (err) => {
        this.connectPromise = null;
        reject(err);
      });
      sock.on("close", () => {
        this.socket = null;
        this.connectPromise = null;
        // Fail outstanding requests so callers don't hang.
        for (const [id, p] of this.pending) {
          clearTimeout(p.timeoutHandle);
          p.reject(new Error(`IPC socket closed before response for ${id}`));
          this.pending.delete(id);
        }
      });
    });
    return this.connectPromise;
  }

  /** Length-prefix framed JSON: 4-byte big-endian length, then UTF-8 payload. */
  private onData(chunk: Buffer): void {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (this.buffer.length >= 4) {
      const len = this.buffer.readUInt32BE(0);
      if (this.buffer.length < 4 + len) return;
      const payload = this.buffer.subarray(4, 4 + len).toString("utf8");
      this.buffer = this.buffer.subarray(4 + len);
      try {
        const msg = JSON.parse(payload) as IpcResponse;
        const p = this.pending.get(msg.id);
        if (p) {
          clearTimeout(p.timeoutHandle);
          this.pending.delete(msg.id);
          p.resolve(msg);
        }
      } catch (err) {
        // Malformed frame — drop it; the caller will time out.
        console.error("[mneme-mcp] malformed IPC frame", err);
      }
    }
  }

  async request<T>(method: string, params: unknown): Promise<IpcResponse<T>> {
    await this.ensureConnected();
    const id = randomUUID();
    const req: IpcRequest = { id, method, params };
    const payload = Buffer.from(JSON.stringify(req), "utf8");
    const header = Buffer.alloc(4);
    header.writeUInt32BE(payload.length, 0);

    return new Promise<IpcResponse<T>>((resolve, reject) => {
      const timeoutHandle = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`IPC timeout: ${method} (${this.REQUEST_TIMEOUT_MS}ms)`));
      }, this.REQUEST_TIMEOUT_MS);

      this.pending.set(id, {
        resolve: (resp) => resolve(resp as IpcResponse<T>),
        reject,
        startedAt: Date.now(),
        timeoutHandle,
      });

      const sock = this.socket;
      if (!sock) {
        clearTimeout(timeoutHandle);
        this.pending.delete(id);
        reject(new Error("IPC socket missing after connect"));
        return;
      }
      sock.write(Buffer.concat([header, payload]), (err) => {
        if (err) {
          clearTimeout(timeoutHandle);
          this.pending.delete(id);
          reject(err);
        }
      });
    });
  }

  private async ensureConnected(): Promise<void> {
    if (this.socket && !this.socket.destroyed) return;

    // First attempt — if the pipe is simply missing (daemon dead),
    // try to revive it exactly once before entering the reconnect loop.
    // Closes the "mneme is unhealthy / supervisor pipe -NNN not found"
    // self-inflicted failure reported against v0.3.0.
    let autoStarted = false;
    try {
      await this.connect();
      return;
    } catch (err) {
      const msg = (err as { message?: string })?.message ?? String(err);
      // Pipe-not-found patterns: ENOENT on Unix, `cannot find the file`
      // on Windows named pipe.
      const missingPipe =
        msg.includes("ENOENT") ||
        msg.includes("cannot find") ||
        msg.includes("No such file");
      if (missingPipe && !autoStarted) {
        console.error(
          "[mneme-mcp] supervisor pipe missing — attempting to start daemon...",
        );
        autoStarted = true;
        await this.spawnDaemonAndWait();
        try {
          await this.connect();
          return;
        } catch {
          // fall through to the reconnect loop below for a second chance.
        }
      }
    }

    while (this.reconnectAttempts < this.MAX_RECONNECT) {
      try {
        await this.connect();
        return;
      } catch (err) {
        this.reconnectAttempts++;
        const backoff = Math.min(1000 * 2 ** this.reconnectAttempts, 5000);
        await new Promise((r) => setTimeout(r, backoff));
        if (this.reconnectAttempts >= this.MAX_RECONNECT) {
          console.error(
            "[mneme-mcp] could not reach the mneme daemon after retries.\n" +
              "  Try: mneme daemon start\n" +
              "  Pipe: " +
              this.socketPath,
          );
          throw err;
        }
      }
    }
  }

  /** Spawn `mneme daemon start` detached and wait for the pipe to appear. */
  private async spawnDaemonAndWait(): Promise<void> {
    try {
      const child = spawn("mneme", ["daemon", "start"], {
        detached: true,
        stdio: "ignore",
        windowsHide: true,
      });
      child.unref();
    } catch (err) {
      console.error("[mneme-mcp] spawn mneme daemon failed:", err);
      return;
    }
    // Give the supervisor up to 5s to come up and write its pipe.
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) {
      await new Promise((r) => setTimeout(r, 250));
      try {
        await new Promise<void>((resolve, reject) => {
          const probe = createConnection(this.socketPath, () => {
            probe.end();
            resolve();
          });
          probe.on("error", reject);
        });
        return;
      } catch {
        // keep waiting
      }
    }
  }

  close(): void {
    if (this.socket) this.socket.destroy();
    this.socket = null;
  }
}

const _client = new IpcClient(discoverSocketPath());

// ---------------------------------------------------------------------------
// Public typed surface — mirrors §13.5 sub-layers
// ---------------------------------------------------------------------------

/** Sub-layer 1: BUILDER — provision a shard for a project. */
export const builder = {
  async buildOrMigrate(projectId: string): Promise<{ shard: string; created: boolean }> {
    const r = await _client.request<{ shard: string; created: boolean }>(
      "builder.build_or_migrate",
      { project_id: projectId },
    );
    return unwrap(r);
  },
};

/** Sub-layer 2: FINDER — locate a shard by cwd or hash. */
export const finder = {
  async findByCwd(cwd: string): Promise<{ project_id: string; shard: string } | null> {
    const r = await _client.request<{ project_id: string; shard: string } | null>(
      "finder.find_by_cwd",
      { cwd },
    );
    return r.ok ? r.data ?? null : null;
  },
  async findByHash(hash: string): Promise<{ project_id: string; shard: string } | null> {
    const r = await _client.request<{ project_id: string; shard: string } | null>(
      "finder.find_by_hash",
      { hash },
    );
    return r.ok ? r.data ?? null : null;
  },
};

/** Sub-layer 3: ACCESS PATH — resolve disk path of a layer's DB file. */
export const path = {
  async shardDb(projectId: string, layer: DbLayer): Promise<string> {
    const r = await _client.request<{ path: string }>("path.shard_db", {
      project_id: projectId,
      layer,
    });
    return unwrap(r).path;
  },
};

/** Sub-layer 4: QUERY — typed read against a shard. */
export const query = {
  async select<T = unknown>(
    layer: DbLayer,
    where: string,
    params: unknown[] = [],
    projectId?: string,
  ): Promise<T[]> {
    const r = await _client.request<T[]>("query.select", {
      layer,
      where,
      params,
      project_id: projectId,
    });
    return unwrap(r);
  },
  async semanticSearch<T = unknown>(
    layer: DbLayer,
    query: string,
    limit = 10,
    projectId?: string,
  ): Promise<T[]> {
    const r = await _client.request<T[]>("query.semantic_search", {
      layer,
      query,
      limit,
      project_id: projectId,
    });
    return unwrap(r);
  },
  async raw<T = unknown>(method: string, params: unknown): Promise<T> {
    return unwrap(await _client.request<T>(method, params));
  },
};

/** Sub-layer 6: INJECT — typed writes through the single-writer task. */
export interface InjectOptions {
  idempotency_key?: string;
  emit_event?: boolean;
  audit?: boolean;
  timeout_ms?: number;
}

export const inject = {
  async insert<T extends Record<string, unknown>>(
    layer: DbLayer,
    row: T,
    opts: InjectOptions = {},
  ): Promise<{ row_id: string }> {
    const r = await _client.request<{ row_id: string }>("inject.insert", {
      layer,
      row,
      opts,
    });
    return unwrap(r);
  },
  async upsert<T extends Record<string, unknown>>(
    layer: DbLayer,
    row: T,
    opts: InjectOptions = {},
  ): Promise<{ row_id: string; created: boolean }> {
    const r = await _client.request<{ row_id: string; created: boolean }>(
      "inject.upsert",
      { layer, row, opts },
    );
    return unwrap(r);
  },
  async update<T extends Record<string, unknown>>(
    layer: DbLayer,
    id: string,
    patch: T,
    opts: InjectOptions = {},
  ): Promise<void> {
    unwrap(await _client.request("inject.update", { layer, id, patch, opts }));
  },
  async delete(layer: DbLayer, id: string, opts: InjectOptions = {}): Promise<void> {
    unwrap(await _client.request("inject.delete", { layer, id, opts }));
  },
  async batch(
    ops: Array<{ layer: DbLayer; op: "insert" | "upsert" | "update" | "delete"; row: unknown }>,
    opts: InjectOptions = {},
  ): Promise<{ applied: number }> {
    return unwrap(
      await _client.request<{ applied: number }>("inject.batch", { ops, opts }),
    );
  },
};

/** Sub-layer 7: LIFECYCLE — snapshot, restore, vacuum, integrity check. */
export const lifecycle = {
  async snapshot(projectId?: string, label?: string): Promise<{ snapshot_id: string; size_bytes: number; created_at: string }> {
    return unwrap(
      await _client.request<{ snapshot_id: string; size_bytes: number; created_at: string }>(
        "lifecycle.snapshot",
        { project_id: projectId, label },
      ),
    );
  },
  async restore(projectId: string, snapshotId: string): Promise<void> {
    unwrap(
      await _client.request("lifecycle.restore", {
        project_id: projectId,
        snapshot_id: snapshotId,
      }),
    );
  },
  async listSnapshots(projectId?: string): Promise<Array<{ snapshot_id: string; created_at: string; size_bytes: number; label: string | null }>> {
    return unwrap(
      await _client.request<Array<{ snapshot_id: string; created_at: string; size_bytes: number; label: string | null }>>(
        "lifecycle.list_snapshots",
        { project_id: projectId },
      ),
    );
  },
  async vacuum(projectId?: string): Promise<{ bytes_freed: number }> {
    return unwrap(
      await _client.request<{ bytes_freed: number }>("lifecycle.vacuum", {
        project_id: projectId,
      }),
    );
  },
  async integrityCheck(projectId?: string): Promise<{ ok: boolean; issues: string[] }> {
    return unwrap(
      await _client.request<{ ok: boolean; issues: string[] }>(
        "lifecycle.integrity_check",
        { project_id: projectId },
      ),
    );
  },
  async rebuild(scope: "graph" | "semantic" | "all"): Promise<{ rebuilt: string[]; duration_ms: number }> {
    return unwrap(
      await _client.request<{ rebuilt: string[]; duration_ms: number }>(
        "lifecycle.rebuild",
        { scope },
      ),
    );
  },
};

/** Live-bus publish — fire-and-forget event emission. */
export const livebus = {
  async emit(topic: string, payload: unknown): Promise<void> {
    try {
      await _client.request("livebus.emit", { topic, payload });
    } catch (err) {
      // Live bus is best-effort; never let emission failures break the caller.
      console.error("[mneme-mcp] livebus emit failed", err);
    }
  },
};

/** Convenience: high-level facade for the 6 hook commands. */
export const hookCmd = {
  async sessionPrime(args: { project: string; sessionId: string }): Promise<{ additional_context: string }> {
    return unwrap(
      await _client.request<{ additional_context: string }>("hook.session_prime", args),
    );
  },
  async inject(args: { prompt: string; sessionId: string; cwd: string }): Promise<{ additional_context: string }> {
    return unwrap(
      await _client.request<{ additional_context: string }>("hook.inject", args),
    );
  },
  async preTool(args: { tool: string; params: unknown; sessionId: string }): Promise<{ skip?: boolean; result?: string; additional_context?: string }> {
    return unwrap(await _client.request("hook.pre_tool", args));
  },
  async postTool(args: { tool: string; resultPath: string; sessionId: string }): Promise<void> {
    unwrap(await _client.request("hook.post_tool", args));
  },
  async turnEnd(args: { sessionId: string }): Promise<void> {
    unwrap(await _client.request("hook.turn_end", args));
  },
  async sessionEnd(args: { sessionId: string }): Promise<void> {
    unwrap(await _client.request("hook.session_end", args));
  },
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function unwrap<T>(r: IpcResponse<T>): T {
  if (!r.ok) {
    const err = r.error;
    const msg = err ? `${err.code}: ${err.message}` : "Unknown IPC error";
    throw new DbError(msg, err?.code ?? "UNKNOWN", err?.detail);
  }
  if (r.data === undefined) {
    throw new DbError("IPC response missing data", "EMPTY_RESPONSE");
  }
  return r.data;
}

export class DbError extends Error {
  constructor(
    message: string,
    public code: string,
    public detail?: unknown,
  ) {
    super(message);
    this.name = "DbError";
  }
}

// Re-export for convenience.
export type { Decision, Finding, Step };

// Test hook: allow integration tests to swap in a mock client.
export function _setClient(_mockClient: unknown): void {
  // The mock should implement IpcClient's `request` method shape.
  // We deliberately do not export the class; rebinding is for test code only.
  Object.assign(_client, _mockClient as object);
}

export function shutdown(): void {
  _client.close();
}
