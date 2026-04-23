// vision/src/api/graph.ts
//
// Client-side shard API for the Vision app.
//
// These helpers hit the Bun server's /api/graph/* endpoints, which in turn
// read the project's SQLite shards directly via `bun:sqlite` (see
// `vision/server/shard.ts`). The server discovery logic mirrors the pattern
// used by `mcp/src/store.ts` — derive ProjectId from the cwd, open the
// shard `.db` read-only, answer scoped queries.
//
// Kept client-only so Vite does not try to bundle `bun:sqlite` into the
// browser chunk. The matching server-side module is `vision/server/shard.ts`.

import type { GraphNode, GraphEdge } from "../api";

/* -------------------------------------------------------------------------- */
/*  Shared types                                                               */
/* -------------------------------------------------------------------------- */

export interface ShardFileRow {
  path: string;
  language: string | null;
  line_count: number | null;
  byte_count: number | null;
  last_parsed_at: string | null;
}

export interface ShardFindingRow {
  id: number;
  rule_id: string;
  scanner: string;
  severity: "critical" | "high" | "medium" | "low" | string;
  file: string;
  line_start: number;
  line_end: number;
  message: string;
  suggestion: string | null;
  created_at: string;
}

export interface GraphStatsPayload {
  ok: boolean;
  project: string | null;
  shardRoot: string | null;
  nodes: number;
  edges: number;
  files: number;
  byKind: Record<string, number>;
  lastIndexAt: string | null;
  error?: string;
}

export interface DaemonHealthPayload {
  ok: boolean;
  status: "running" | "missing" | "error";
  url: string;
  detail?: string;
  error?: string;
}

export interface NodesResponse {
  nodes: GraphNode[];
  error?: string;
}
export interface EdgesResponse {
  edges: GraphEdge[];
  error?: string;
}
export interface FilesResponse {
  files: ShardFileRow[];
  error?: string;
}
export interface FindingsResponse {
  findings: ShardFindingRow[];
  error?: string;
}

/* -------------------------------------------------------------------------- */
/*  Client fetchers                                                            */
/* -------------------------------------------------------------------------- */

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal });
  if (!res.ok) throw new Error(`${url} -> HTTP ${res.status}`);
  return (await res.json()) as T;
}

export async function fetchNodes(signal?: AbortSignal, limit = 2000): Promise<NodesResponse> {
  return getJson<NodesResponse>(`/api/graph/nodes?limit=${limit}`, signal);
}

export async function fetchEdges(signal?: AbortSignal, limit = 8000): Promise<EdgesResponse> {
  return getJson<EdgesResponse>(`/api/graph/edges?limit=${limit}`, signal);
}

export async function fetchFiles(signal?: AbortSignal, limit = 2000): Promise<FilesResponse> {
  return getJson<FilesResponse>(`/api/graph/files?limit=${limit}`, signal);
}

export async function fetchFindings(
  signal?: AbortSignal,
  limit = 2000,
): Promise<FindingsResponse> {
  return getJson<FindingsResponse>(`/api/graph/findings?limit=${limit}`, signal);
}

export async function fetchStatus(signal?: AbortSignal): Promise<GraphStatsPayload> {
  return getJson<GraphStatsPayload>("/api/graph/status", signal);
}

export async function fetchDaemonHealth(
  signal?: AbortSignal,
): Promise<DaemonHealthPayload> {
  return getJson<DaemonHealthPayload>("/api/daemon/health", signal);
}
