// Vision web server — runs under Bun (`bun server.ts`).
// Binds to 127.0.0.1 only (NEVER 0.0.0.0). Serves built dist/, proxies /api/graph
// to the datatree IPC, and forwards a WebSocket /ws upgrade to the livebus.
//
// Local-only by policy. Voice nav is stubbed for v1 (Phase 5 ships real voice).

import { file } from "bun";
import { join, normalize, sep } from "node:path";
import {
  fetchGraphNodes,
  fetchGraphEdges,
  fetchFilesForTreemap,
  fetchFindings,
  buildStatusPayload,
  probeDaemon,
} from "./server/shard";

const HOST = "127.0.0.1";
const PORT = Number(process.env.VISION_PORT ?? 7777);
const DIST_DIR = join(import.meta.dir, "dist");

// Backend services the server proxies to.
const DATATREE_IPC = process.env.DATATREE_IPC ?? "http://127.0.0.1:7780";
const LIVEBUS_WS = process.env.LIVEBUS_WS ?? "ws://127.0.0.1:7778/ws";
const DAEMON_HEALTH = process.env.DAEMON_HEALTH ?? "http://127.0.0.1:7777/health";

interface ProxyEnvelope {
  view: string;
  query: Record<string, string>;
}

function jsonResponse(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...corsHeaders(),
    },
  });
}

function corsHeaders(): HeadersInit {
  // Local-only; we still set CORS so the Tauri webview can load resources.
  return {
    "access-control-allow-origin": "http://127.0.0.1",
    "access-control-allow-methods": "GET, POST, OPTIONS",
    "access-control-allow-headers": "content-type",
  };
}

function safeStaticPath(rawPath: string): string | null {
  // Strip query string + leading slash, then prevent directory traversal.
  const clean = rawPath.split("?")[0]?.replace(/^\/+/, "") ?? "";
  const normalized = normalize(clean).replace(/^(\.\.[/\\])+/, "");
  if (normalized.startsWith("..") || normalized.includes(`..${sep}`)) return null;
  return join(DIST_DIR, normalized || "index.html");
}

async function serveStatic(pathname: string): Promise<Response> {
  const target = safeStaticPath(pathname);
  if (!target) return new Response("forbidden", { status: 403 });
  const f = file(target);
  if (await f.exists()) return new Response(f);
  // SPA fallback — every non-asset URL returns the index.
  const index = file(join(DIST_DIR, "index.html"));
  if (await index.exists()) return new Response(index);
  return new Response("not built — run `vite build`", { status: 404 });
}

// Direct shard reader for a single view. Mirrors the datatree IPC
// shape — the fallback path when IPC is unreachable or not started.
function serveViewFromShard(view: string, url: URL): Response {
  const limit = Number(url.searchParams.get("limit") ?? "2000");
  try {
    if (view === "force-galaxy") {
      const nodes = fetchGraphNodes(limit);
      const edges = fetchGraphEdges(limit * 4);
      return jsonResponse({
        view,
        nodes,
        edges,
        meta: { source: "shard", node_count: nodes.length, edge_count: edges.length },
      });
    }
    if (view === "treemap") {
      const files = fetchFilesForTreemap(limit);
      // Views expect GraphNode shape with label/size; re-use the same envelope.
      const nodes = files.map((f) => ({
        id: f.path,
        label: f.path,
        type: f.language ?? "file",
        size: Math.max(1, Math.ceil((f.line_count ?? 1) / 50)),
        meta: {
          language: f.language,
          line_count: f.line_count,
          byte_count: f.byte_count,
        },
      }));
      return jsonResponse({
        view,
        nodes,
        edges: [],
        meta: { source: "shard", file_count: files.length },
      });
    }
    if (view === "risk-dashboard") {
      const findings = fetchFindings(limit);
      const nodes = findings.map((f) => ({
        id: `${f.file}:${f.line_start}:${f.rule_id}`,
        label: `${f.file} (${f.rule_id})`,
        type: f.severity,
        size: severityToSize(f.severity),
        meta: {
          rule_id: f.rule_id,
          scanner: f.scanner,
          severity: f.severity,
          message: f.message,
          file: f.file,
          line_start: f.line_start,
          line_end: f.line_end,
          risk: severityToRisk(f.severity),
        },
      }));
      return jsonResponse({
        view,
        nodes,
        edges: [],
        meta: { source: "shard", finding_count: findings.length },
      });
    }
  } catch (err) {
    return jsonResponse(
      { view, nodes: [], edges: [], meta: { source: "shard", error: String(err) } },
      200,
    );
  }
  // No shard-aware handler for this view yet — signal not-implemented so
  // client falls back to IPC / placeholder.
  return jsonResponse(
    { view, nodes: [], edges: [], meta: { source: "shard", unsupported: true } },
    200,
  );
}

function severityToSize(sev: string): number {
  switch (sev) {
    case "critical":
      return 10;
    case "high":
      return 8;
    case "medium":
      return 5;
    case "low":
      return 3;
    default:
      return 2;
  }
}

function severityToRisk(sev: string): number {
  switch (sev) {
    case "critical":
      return 95;
    case "high":
      return 75;
    case "medium":
      return 45;
    case "low":
      return 20;
    default:
      return 10;
  }
}

async function proxyGraph(req: Request, url: URL): Promise<Response> {
  const view = url.searchParams.get("view") ?? "force-galaxy";
  const query: Record<string, string> = {};
  for (const [k, v] of url.searchParams.entries()) query[k] = v;
  const envelope: ProxyEnvelope = { view, query };

  // If the caller explicitly asks for the shard path (?source=shard) skip IPC.
  if (query["source"] === "shard") {
    return serveViewFromShard(view, url);
  }

  try {
    const upstream = await fetch(`${DATATREE_IPC}/graph`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(envelope),
    });
    const text = await upstream.text();
    return new Response(text, {
      status: upstream.status,
      headers: {
        "content-type": upstream.headers.get("content-type") ?? "application/json",
        ...corsHeaders(),
      },
    });
  } catch {
    // IPC unreachable — degrade to direct shard read. The three views wired
    // in review P3 read from bun:sqlite, so this is a first-class fallback,
    // not a placeholder.
    return serveViewFromShard(view, url);
  }
}

const server = Bun.serve({
  hostname: HOST,
  port: PORT,
  development: process.env.NODE_ENV !== "production",

  async fetch(req, srv) {
    const url = new URL(req.url);

    // CORS preflight
    if (req.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: corsHeaders() });
    }

    // WebSocket upgrade for /ws — proxied to livebus.
    if (url.pathname === "/ws") {
      const upgraded = srv.upgrade(req, { data: { livebusUrl: LIVEBUS_WS } });
      if (upgraded) return undefined as unknown as Response;
      return new Response("upgrade required", { status: 426 });
    }

    if (url.pathname === "/api/health") {
      return jsonResponse({
        ok: true,
        host: HOST,
        port: PORT,
        datatreeIpc: DATATREE_IPC,
        livebusWs: LIVEBUS_WS,
        ts: Date.now(),
      });
    }

    if (url.pathname === "/api/graph") {
      return proxyGraph(req, url);
    }

    // Direct shard endpoints — local-only bun:sqlite reads.
    if (url.pathname === "/api/graph/nodes") {
      try {
        const limit = Number(url.searchParams.get("limit") ?? "2000");
        return jsonResponse({ nodes: fetchGraphNodes(limit) });
      } catch (err) {
        return jsonResponse({ nodes: [], error: String(err) }, 200);
      }
    }
    if (url.pathname === "/api/graph/edges") {
      try {
        const limit = Number(url.searchParams.get("limit") ?? "8000");
        return jsonResponse({ edges: fetchGraphEdges(limit) });
      } catch (err) {
        return jsonResponse({ edges: [], error: String(err) }, 200);
      }
    }
    if (url.pathname === "/api/graph/files") {
      try {
        const limit = Number(url.searchParams.get("limit") ?? "2000");
        return jsonResponse({ files: fetchFilesForTreemap(limit) });
      } catch (err) {
        return jsonResponse({ files: [], error: String(err) }, 200);
      }
    }
    if (url.pathname === "/api/graph/findings") {
      try {
        const limit = Number(url.searchParams.get("limit") ?? "2000");
        return jsonResponse({ findings: fetchFindings(limit) });
      } catch (err) {
        return jsonResponse({ findings: [], error: String(err) }, 200);
      }
    }
    if (url.pathname === "/api/graph/status") {
      return jsonResponse(buildStatusPayload());
    }
    if (url.pathname === "/api/daemon/health") {
      const probe = await probeDaemon(DAEMON_HEALTH);
      return jsonResponse(probe);
    }

    // Voice nav stub (§9.6) — real implementation lands in Phase 5.
    if (url.pathname === "/api/voice") {
      return jsonResponse({ enabled: false, phase: "stub", message: "voice nav not yet wired" });
    }

    return serveStatic(url.pathname);
  },

  websocket: {
    open(ws) {
      // Connect to upstream livebus and pipe both directions.
      const data = ws.data as { livebusUrl: string; upstream?: WebSocket };
      try {
        const upstream = new WebSocket(data.livebusUrl);
        data.upstream = upstream;
        upstream.addEventListener("message", (event) => {
          try {
            ws.send(typeof event.data === "string" ? event.data : new Uint8Array(event.data as ArrayBuffer));
          } catch {
            /* client closed */
          }
        });
        upstream.addEventListener("close", () => {
          try {
            ws.close();
          } catch {
            /* noop */
          }
        });
        upstream.addEventListener("error", () => {
          try {
            ws.send(JSON.stringify({ type: "livebus:error", message: "upstream unavailable" }));
          } catch {
            /* noop */
          }
        });
      } catch (err) {
        ws.send(JSON.stringify({ type: "livebus:error", message: String(err) }));
      }
    },
    message(ws, message) {
      const data = ws.data as { upstream?: WebSocket };
      const upstream = data.upstream;
      if (!upstream || upstream.readyState !== WebSocket.OPEN) return;
      upstream.send(typeof message === "string" ? message : new Uint8Array(message));
    },
    close(ws) {
      const data = ws.data as { upstream?: WebSocket };
      try {
        data.upstream?.close();
      } catch {
        /* noop */
      }
    },
  },
});

// eslint-disable-next-line no-console
console.log(`[vision] http://${server.hostname}:${server.port}  (local-only)`);
