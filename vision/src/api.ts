// API helpers for the Vision app. Wraps /api/graph and friends in tanstack-query
// style fetchers (we keep them framework-agnostic so views can call them directly).

import type { ViewId } from "./views";

export interface GraphNode {
  id: string;
  label?: string;
  type?: string;
  size?: number;
  x?: number;
  y?: number;
  z?: number;
  color?: string;
  meta?: Record<string, unknown>;
}

export interface GraphEdge {
  id?: string;
  source: string;
  target: string;
  weight?: number;
  type?: string;
  meta?: Record<string, unknown>;
}

export interface GraphPayload {
  view: ViewId | string;
  nodes: GraphNode[];
  edges: GraphEdge[];
  meta?: Record<string, unknown>;
}

export interface FetchOptions {
  signal?: AbortSignal;
  params?: Record<string, string | number | boolean>;
}

const DEFAULT_HEADERS: HeadersInit = { "content-type": "application/json" };

function buildUrl(path: string, params: Record<string, string | number | boolean> = {}): string {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) qs.set(k, String(v));
  const suffix = qs.toString();
  return suffix ? `${path}?${suffix}` : path;
}

export async function fetchGraph(view: ViewId, options: FetchOptions = {}): Promise<GraphPayload> {
  const url = buildUrl("/api/graph", { view, ...(options.params ?? {}) });
  try {
    const res = await fetch(url, { signal: options.signal, headers: DEFAULT_HEADERS });
    if (!res.ok) throw new Error(`graph fetch failed: ${res.status}`);
    const json = (await res.json()) as GraphPayload;
    return json;
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    // Return placeholder data so the UI renders even when the daemon is offline.
    return placeholderPayload(view, err);
  }
}

export async function fetchHealth(): Promise<{ ok: boolean; ts: number }> {
  try {
    const res = await fetch("/api/health");
    return (await res.json()) as { ok: boolean; ts: number };
  } catch {
    return { ok: false, ts: Date.now() };
  }
}

// Build a deterministic placeholder so views render before the daemon answers.
export function placeholderPayload(view: ViewId | string, err?: unknown): GraphPayload {
  const nodes: GraphNode[] = Array.from({ length: 32 }, (_, i) => ({
    id: `n${i}`,
    label: `node-${i}`,
    type: ["module", "page", "store", "util"][i % 4],
    size: 4 + (i % 7),
    x: Math.cos((i / 32) * Math.PI * 2) * 200,
    y: Math.sin((i / 32) * Math.PI * 2) * 200,
    color: `hsl(${(i * 31) % 360} 70% 60%)`,
    meta: { placeholder: true },
  }));
  const edges: GraphEdge[] = nodes.slice(1).map((n, i) => ({
    id: `e${i}`,
    source: nodes[i]?.id ?? "n0",
    target: n.id,
    weight: 1,
  }));
  return {
    view,
    nodes,
    edges,
    meta: { placeholder: true, error: err ? String(err) : null },
  };
}
