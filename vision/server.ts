// Vision web server — runs under Bun (`bun server.ts`).
// Binds to 127.0.0.1 only (NEVER 0.0.0.0). Serves built dist/, proxies /api/graph
// to the datatree IPC, and forwards a WebSocket /ws upgrade to the livebus.
//
// Local-only by policy. Voice nav is stubbed for v1 (Phase 5 ships real voice).

import { file } from "bun";
import { join, normalize, sep } from "node:path";

const HOST = "127.0.0.1";
const PORT = Number(process.env.VISION_PORT ?? 7777);
const DIST_DIR = join(import.meta.dir, "dist");

// Backend services the server proxies to.
const DATATREE_IPC = process.env.DATATREE_IPC ?? "http://127.0.0.1:7780";
const LIVEBUS_WS = process.env.LIVEBUS_WS ?? "ws://127.0.0.1:7778/ws";

interface ProxyEnvelope {
  view: string;
  query: Record<string, string>;
}

function jsonResponse(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json; charset=utf-8" },
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

async function proxyGraph(req: Request, url: URL): Promise<Response> {
  const view = url.searchParams.get("view") ?? "force-galaxy";
  const query: Record<string, string> = {};
  for (const [k, v] of url.searchParams.entries()) query[k] = v;
  const envelope: ProxyEnvelope = { view, query };
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
  } catch (err) {
    // Fallback to placeholder data when datatree IPC is unreachable so the UI still renders.
    return jsonResponse(
      {
        view,
        nodes: [],
        edges: [],
        meta: { fallback: true, reason: String(err) },
      },
      200,
    );
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
