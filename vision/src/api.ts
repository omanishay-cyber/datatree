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

// F1 D2 fix: in production Tauri the SPA loads via the `tauri://` custom
// protocol. Relative URLs like `/api/graph/nodes` resolve to
// `tauri://localhost/api/...` and Tauri's SPA fallback returns the
// bundled `index.html` for unknown paths — so every fetch parses HTML
// as JSON and the entire dashboard fills with `Unexpected token '<'`
// toasts. Detect Tauri at module-init and prepend the daemon's HTTP
// origin in that case. In `bun server.ts` dev mode the page is served
// from the same origin as the API, so the empty prefix is correct.
export const API_BASE: string = (() => {
  if (typeof window !== "undefined") {
    const w = window as unknown as { __TAURI__?: unknown; __TAURI_INTERNALS__?: unknown };
    if (w.__TAURI__ || w.__TAURI_INTERNALS__) {
      return "http://127.0.0.1:7777";
    }
  }
  return "";
})();

function buildUrl(path: string, params: Record<string, string | number | boolean> = {}): string {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) qs.set(k, String(v));
  const suffix = qs.toString();
  const rel = suffix ? `${path}?${suffix}` : path;
  // Absolute URL when in Tauri (API_BASE='http://127.0.0.1:7777'),
  // relative when in Bun-served dev (API_BASE='').
  return API_BASE + rel;
}

/**
 * HIGH-41 fix (2026-05-05 audit): the previous implementation was a
 * pure type assertion `(await res.json()) as GraphPayload`. If the
 * daemon returned `{}` or `null` (e.g. a 200 with empty body during
 * a fresh install or a partial response from a misbehaving proxy),
 * downstream `.nodes` / `.edges` reads would throw at runtime with
 * "Cannot read property of undefined". sibling api/graph.ts goes to
 * extreme lengths with isPlainObject + Array.isArray for the same
 * surface; api.ts was the unguarded twin.
 *
 * Validate the shape before returning. Fall back to placeholderPayload
 * on malformed responses just like network errors already do.
 */
function isPlainObject(x: unknown): x is Record<string, unknown> {
  return typeof x === "object" && x !== null && !Array.isArray(x);
}

function looksLikeGraphPayload(x: unknown): x is GraphPayload {
  return isPlainObject(x) && Array.isArray(x.nodes) && Array.isArray(x.edges);
}

export async function fetchGraph(view: ViewId, options: FetchOptions = {}): Promise<GraphPayload> {
  const url = buildUrl("/api/graph", { view, ...(options.params ?? {}) });
  try {
    const res = await fetch(url, { signal: options.signal, headers: DEFAULT_HEADERS });
    if (!res.ok) throw new Error(`graph fetch failed: ${res.status}`);
    const json: unknown = await res.json();
    if (!looksLikeGraphPayload(json)) {
      // Daemon returned an unexpected shape (empty {}, null, an
      // error envelope, etc.). Treat as a soft failure so views
      // still render the placeholder rather than crashing on
      // `.nodes` access.
      return placeholderPayload(view, new Error("daemon response missing nodes/edges"));
    }
    return json;
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    // Return placeholder data so the UI renders even when the daemon is offline.
    return placeholderPayload(view, err);
  }
}

/**
 * HIGH-41 fix: same defensive-narrowing treatment as fetchGraph.
 * The /api/health response is small + stable, but if a misconfigured
 * proxy returns HTML or the daemon is mid-restart returning a
 * partial body, we want false-with-Date.now()-ts instead of an
 * unhandled exception that breaks the SPA's daemon-ok indicator.
 */
function looksLikeHealthPayload(x: unknown): x is { ok: boolean; ts: number } {
  return (
    isPlainObject(x) && typeof x.ok === "boolean" && typeof x.ts === "number"
  );
}

export async function fetchHealth(): Promise<{ ok: boolean; ts: number }> {
  try {
    const res = await fetch(API_BASE + "/api/health");
    if (!res.ok) return { ok: false, ts: Date.now() };
    const json: unknown = await res.json();
    if (!looksLikeHealthPayload(json)) {
      return { ok: false, ts: Date.now() };
    }
    return json;
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
